//! Calling the page's probes (`probes.js`) from Rust. A webview evaluates a script but cannot hand
//! back what a promise resolves to, so a probe is started under an id and its result fetched by
//! polling `take(id)` until it is there.

use std::fmt::Write as _;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde_json::Value;
use tauri::{Runtime, WebviewWindow};

/// How long one probe may take (a first visit loads code and data).
const TIMEOUT: Duration = Duration::from_secs(30);
/// How often its result is asked for. Probes time themselves, so this only delays the next step.
const POLL: Duration = Duration::from_millis(10);

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// Runs `probe(args...)` in the page and waits for what it resolved to; Err with what it threw.
pub(crate) fn call<R: Runtime>(window: &WebviewWindow<R>, probe: &str, args: &[Value]) -> Result<Value, String> {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let mut start = format!("window.__gitpromptPerf.start({id}, {}", Value::from(probe));
    for arg in args {
        let _ = write!(start, ", {arg}");
    }
    start.push(')');
    window.eval(start).map_err(|e| format!("could not start {probe}: {e}"))?;
    let deadline = Instant::now() + TIMEOUT;
    while Instant::now() < deadline {
        if let Some(result) = take(window, id)? {
            return outcome(probe, result);
        }
        std::thread::sleep(POLL);
    }
    Err(format!("{probe} gave no result within {} s", TIMEOUT.as_secs()))
}

/// The result of probe `id` once it is there (`{value}` or `{error}`), None while it runs.
fn take<R: Runtime>(window: &WebviewWindow<R>, id: u64) -> Result<Option<Value>, String> {
    let (send, receive) = mpsc::channel();
    window
        .eval_with_callback(format!("window.__gitpromptPerf.take({id})"), move |json| {
            let _ = send.send(json);
        })
        .map_err(|e| format!("could not ask the page: {e}"))?;
    let json = receive.recv_timeout(TIMEOUT).map_err(|_| "the page did not answer".to_owned())?;
    let result: Value = serde_json::from_str(&json).unwrap_or(Value::Null);
    Ok(if result.is_object() { Some(result) } else { None })
}

fn outcome(probe: &str, mut result: Value) -> Result<Value, String> {
    match result.get("error").and_then(Value::as_str) {
        Some(error) => Err(format!("{probe}: {error}")),
        None => Ok(result.get_mut("value").map_or(Value::Null, Value::take)),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn a_value_or_what_the_page_threw() {
        assert_eq!(outcome("firstLoad", json!({ "value": { "ready": 120 } })).unwrap(), json!({ "ready": 120 }));
        assert_eq!(outcome("settled", json!({})).unwrap(), Value::Null);
        let error = outcome("switchTo", json!({ "error": "no sidebar link to /x" })).unwrap_err();
        assert_eq!(error, "switchTo: no sidebar link to /x");
    }
}
