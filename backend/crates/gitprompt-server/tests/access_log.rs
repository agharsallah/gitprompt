//! The request log: method, path, status, duration; never the query string or a header value.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::needless_pass_by_value,
    unreachable_pub,
    reason = "integration test: a panic is a test failure, fixtures use small known values, and helpers are crate-private anyway"
)]

mod common;

use std::io::Write;
use std::sync::{Arc, Mutex, PoisonError};

use common::api;
use tracing_subscriber::fmt::MakeWriter;

/// Log lines collected in memory.
#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl Captured {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap_or_else(PoisonError::into_inner)).into_owned()
    }
}

impl Write for Captured {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for Captured {
    type Writer = Captured;
    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

/// Run `f` with a debug-level subscriber writing into the returned buffer (this thread only).
async fn logged<F: std::future::Future<Output = ()>>(f: F) -> String {
    let captured = Captured::default();
    let subscriber =
        tracing_subscriber::fmt().with_writer(captured.clone()).with_ansi(false).with_env_filter("gitprompt_server::http=debug").finish();
    let _guard = tracing::subscriber::set_default(subscriber);
    f.await;
    captured.text()
}

#[tokio::test]
async fn each_request_is_logged_with_method_path_status_and_duration() {
    let api = api();
    let text = logged(async {
        api.get("/api/prompts").send().await;
        api.get("/api/nothing").send().await;
    })
    .await;
    assert!(text.contains("method=GET path=\"/api/prompts\" status=200 ms="), "{text}");
    assert!(text.contains("path=\"/api/nothing\" status=404"), "{text}");
}

#[tokio::test]
async fn query_strings_and_headers_are_never_logged() {
    let api = api();
    let token = api.token();
    let text = logged(async {
        api.get("/api/search").query(&[("q", "my-secret-search")]).send().await;
        api.post("/ingest/claude-code").auth().json(serde_json::json!({"prompt": "hush"})).send().await;
    })
    .await;
    assert!(text.contains("path=\"/api/search\""), "{text}");
    assert!(text.contains("path=\"/ingest/claude-code\""), "{text}");
    for secret in ["my-secret-search", "hush", token.as_str(), "Bearer"] {
        assert!(!text.contains(secret), "{secret} in {text}");
    }
}

#[tokio::test]
async fn requests_refused_by_the_host_check_are_logged_too() {
    let api = api();
    let text = logged(async {
        api.get("/api/prompts").host("evil.example").send().await;
    })
    .await;
    assert!(text.contains("status=400"), "{text}");
}
