//! The application: settings + options -> a [`Service`], a [`Bus`] and the axum router.
//! Port of `api/app.py`.

use std::future::Future;

use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use tokio::net::TcpListener;

use gitprompt_core::Settings;

use crate::bus::Bus;
use crate::error::{ApiError, ApiResult};
use crate::options::AppOptions;
use crate::service::Service;
use crate::{access_log, dashboard, routes, security};

#[derive(Debug)]
pub struct App {
    service: Service,
}

impl App {
    /// Create the receiver token (if there is none), open the store, build the bus. Nothing is
    /// served yet.
    pub fn new(settings: Settings, options: AppOptions) -> ApiResult<Self> {
        let bus = Bus::new(options.bus_backlog);
        Ok(App { service: Service::new(settings, options, bus)? })
    }

    /// The facade the Tauri commands call.
    pub fn service(&self) -> Service {
        self.service.clone()
    }

    pub fn bus(&self) -> Bus {
        self.service.bus().clone()
    }

    /// Request log ([`crate::access_log`]) -> Host check -> API routes (`routes::api`) -> `/healthz`
    /// -> the dashboard (when `AppOptions::dashboard` is set; it catches every other path) -> 404
    /// `{"detail": "Not Found"}`.
    pub fn router(&self) -> Router {
        let router = routes::api(self.service.clone())
            .route("/healthz", get(|| async { Json(serde_json::json!({"ok": true})) }))
            .method_not_allowed_fallback(|| async {
                (StatusCode::METHOD_NOT_ALLOWED, Json(serde_json::json!({"detail": "Method Not Allowed"})))
            });
        let router = match self.service.dashboard() {
            Some(source) => router.fallback_service(dashboard::router(source.clone())),
            None => router.fallback(|| async { ApiError::NotFound("Not Found".into()).into_response() }),
        };
        security::trusted_hosts(router).layer(axum::middleware::from_fn(access_log::log_request))
    }

    /// Serve on `listener` until `shutdown` resolves, then finish in-flight requests and return.
    /// Open SSE streams end at shutdown.
    pub async fn serve(self, listener: TcpListener, shutdown: impl Future<Output = ()> + Send + 'static) -> ApiResult<()> {
        let closing = self.service.closing();
        let signal = async move {
            shutdown.await;
            closing.cancel();
        };
        axum::serve(listener, self.router()).with_graceful_shutdown(signal).await.map_err(|e| gitprompt_core::Error::Io(e).into())
    }
}
