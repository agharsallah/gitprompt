//! The performance run, on its own thread once the dashboard loaded: the load's timings, page
//! switches through the sidebar, real window resizes, then the results to `GITPROMPT_PERF_OUT`, a
//! quiet spell for whoever measures the idle app, and exit.

use serde_json::{Value, json};
use tauri::{LogicalSize, Manager, Runtime, WebviewWindow};

use super::{Perf, bridge, epoch_ms};

/// The page the run starts and ends on, then the others it switches to, in sidebar order.
const HOME: &str = "/";
const PAGES: &[&str] = &["/prompts", "/sessions", "/projects", "/sources"];
/// The width the window opens at (window.rs), then the widths it goes through; 720 is its smallest.
const OPEN_WIDTH: f64 = 1280.0;
const WIDTHS: &[f64] = &[1024.0, 900.0, 720.0, OPEN_WIDTH];
const HEIGHT: f64 = 840.0;
/// The pages resized: the overview's charts and the longest list.
const RESIZED: &[&str] = &["/", "/prompts"];

pub(crate) fn run<R: Runtime>(window: &WebviewWindow<R>, perf: &Perf) {
    let result = measure(window, perf);
    let report = match &result {
        Ok(report) => report.clone(),
        Err(error) => json!({ "error": error, "marks": perf.marks() }),
    };
    if let Err(e) = perf.write(&report) {
        tracing::error!(error = %e, out = %perf.config.out.display(), "could not write the performance results");
    }
    if result.is_ok() {
        std::thread::sleep(perf.config.idle);
    } else {
        tracing::error!(error = ?result.err(), "the performance run failed");
    }
    window.app_handle().exit(i32::from(report.get("error").is_some()));
}

fn measure<R: Runtime>(window: &WebviewWindow<R>, perf: &Perf) -> Result<Value, String> {
    let load = bridge::call(window, "firstLoad", &[])?;
    let switches = switching(window, perf.config.rounds)?;
    let resizes = resizing(window)?;
    bridge::call(window, "switchTo", &[json!(HOME)])?;
    let inspect = bridge::call(window, "inspect", &[])?;
    Ok(json!({
        "cold": perf.config.cold,
        "marks": perf.marks(),
        "load": load,
        "switches": switches,
        "resizes": resizes,
        "inspect": inspect,
    }))
}

/// Every other page in turn, then back to the first, `rounds` times.
fn switching<R: Runtime>(window: &WebviewWindow<R>, rounds: usize) -> Result<Vec<Value>, String> {
    let mut visits = Vec::new();
    for round in 0..rounds {
        for &path in PAGES.iter().chain([&HOME]) {
            let timing = bridge::call(window, "switchTo", &[json!(path)])?;
            visits.push(json!({
                "path": path,
                "round": round,
                "firstVisit": round == 0 && path != HOME,
                "ms": timing.get("ms"),
                "longTaskMs": timing.get("longTaskMs"),
            }));
        }
    }
    Ok(visits)
}

/// Each resized page through WIDTHS: the time from asking the window for a size to the page
/// painted at that size (both on the system clock).
fn resizing<R: Runtime>(window: &WebviewWindow<R>) -> Result<Vec<Value>, String> {
    let mut results = Vec::new();
    for path in RESIZED {
        bridge::call(window, "switchTo", &[json!(path)])?;
        let mut from = OPEN_WIDTH;
        for &to in WIDTHS {
            let asked = epoch_ms();
            window.set_size(LogicalSize::new(to, HEIGHT)).map_err(|e| format!("could not resize the window: {e}"))?;
            let painted = bridge::call(window, "resized", &[json!(to)])?;
            let ms = painted.get("paintedAt").and_then(Value::as_f64).map(|at| at - asked);
            results.push(json!({ "path": path, "from": from, "to": to, "ms": ms }));
            from = to;
        }
    }
    Ok(results)
}
