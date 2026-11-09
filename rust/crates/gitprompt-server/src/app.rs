//! The application: settings + options -> a [`Service`], a [`Bus`] and the axum router.
//! Port of `api/app.py`.

use std::future::Future;

use axum::Router;
use tokio::net::TcpListener;

use gitprompt_core::Settings;

use crate::bus::Bus;
use crate::error::ApiResult;
use crate::options::AppOptions;
use crate::service::Service;

pub struct App {
    _service: Service,
}

impl App {
    /// Create the receiver token (if there is none), open the store, build the bus. Nothing is
    /// served yet.
    pub fn new(_settings: Settings, _options: AppOptions) -> ApiResult<Self> {
        todo!()
    }

    /// The facade the Tauri commands call.
    pub fn service(&self) -> Service {
        todo!()
    }

    pub fn bus(&self) -> Bus {
        todo!()
    }

    /// Host check -> API routes (`routes::api`) -> `/healthz` -> the dashboard (when
    /// `AppOptions::dashboard` is set; it catches every other path) -> 404 `{"detail": "Not Found"}`.
    pub fn router(&self) -> Router {
        todo!()
    }

    /// Serve on `listener` until `shutdown` resolves, then finish in-flight requests and return.
    /// Open SSE streams end at shutdown.
    pub async fn serve(self, _listener: TcpListener, _shutdown: impl Future<Output = ()> + Send + 'static) -> ApiResult<()> {
        todo!()
    }
}
