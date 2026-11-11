//! HTTP adapters over [`Service`]: parse, call, serialize. Paths, methods and statuses are the wire
//! contract with the dashboard (`web/src/lib/api/client.ts`) and the hooks:
//!
//! | method | path                                   | guard        | answer                          |
//! |--------|----------------------------------------|--------------|---------------------------------|
//! | POST   | /ingest/{source}                       | token        | 204, empty body                 |
//! | POST   | /ingest/{source}/v1/{logs\|metrics}    | token        | 200 `{}`; 404 for a non-OTLP source |
//! | GET    | /api/prompts                           |              | `[PromptOut]`                   |
//! | GET    | /api/prompts/stream                    |              | SSE, `data: {"prompts":n,"events":m}` |
//! | GET    | /api/prompts/{id}                      |              | `PromptDetail`; 404             |
//! | PUT    | /api/prompts/{id}/annotation           | X-GitPrompt  | `Annotation`; 404               |
//! | POST   | /api/prompts/{id}/redact               | X-GitPrompt  | `{updated}`; 404                |
//! | GET    | /api/sources                           |              | `[SourceOut]`                   |
//! | GET    | /api/sources/recent                    |              | `[RecentCapture]`               |
//! | GET    | /api/sources/problems                  |              | `[SpecProblemOut]`              |
//! | GET    | /api/sources/{source}/spec             |              | `SpecText`; 404                 |
//! | POST   | /`api/sources/{source}/connect?dry_run`  | X-GitPrompt  | `Change`; 409 policy/config/disabled |
//! | POST   | /`api/sources/{source}/disconnect?dry_run` | X-GitPrompt | `Change`                       |
//! | POST   | /api/sources/{source}/enable           | X-GitPrompt  | `{enabled}`; 409 locked by env  |
//! | POST   | /api/sources/{source}/disable          | X-GitPrompt  | `{enabled}`; 409 locked by env  |
//! | POST   | /api/sources/{source}/remap            | X-GitPrompt  | `{updated}`                     |
//! | GET    | /api/stats                             |              | `Stats`; 422 invalid query      |
//! | GET    | /api/stats/telemetry                   |              | `Telemetry`                     |
//! | GET    | /api/fields                            |              | `Catalog`                       |
//! | GET    | /api/settings                          |              | `SettingsOut`                   |
//! | PUT    | /api/settings                          | X-GitPrompt  | `SettingsSaved`; 422 invalid, 409 locked |
//! | GET    | /api/data                              |              | `DataOut`                       |
//! | POST   | /`api/data/compact?dry_run=true`         | X-GitPrompt  | `ReportOut`; 400 days < 1       |
//! | POST   | /api/data/delete-events?dry_run=true   | X-GitPrompt  | `ReportOut`; 400 no source/before |
//! | GET    | /api/security                          |              | `SecurityOut`                   |
//! | POST   | /api/security/rotate-token?dry_run=true| X-GitPrompt  | `Rotation`; 409 blocked         |
//! | GET    | /api/stories/session/{session}         |              | `SessionStory`; 404             |
//! | GET    | /api/stories/project/{project}         |              | `ProjectStory`; 404             |
//! | GET    | /api/stories/prompt/{id}               |              | `Lineage`; 404                  |
//! | GET    | /api/search                            |              | `SearchResults`                 |
//! | GET    | /api/export/{kind}                     |              | the file; 422 kind/id, 404 unknown id |
//! | GET    | /api/export/{kind}/charts              |              | `[ChartInfo]`                   |
//! | GET    | /api/status                            |              | `StatusOut` (no secrets)        |
//! | GET    | /healthz                               |              | `{"ok": true}`                  |
//!
//! Every request also passes the Host check (400). Errors are `{"detail": "..."}`.

mod data;
mod export;
mod extract;
mod fields;
mod ingest;
mod prompts;
mod security;
mod settings;
mod sources;
mod stats;
mod status;
mod stories;

use axum::Router;
use axum::middleware::from_fn;

use crate::service::Service;

/// Every route above, without the Host check and the dashboard (see `App::router`).
pub fn api(service: Service) -> Router {
    let writes = Router::new()
        .merge(prompts::writes())
        .merge(sources::writes())
        .merge(settings::writes())
        .merge(data::writes())
        .merge(security::writes())
        .route_layer(from_fn(crate::security::require_dashboard));
    let ingest = ingest::router().route_layer(axum::middleware::from_fn_with_state(service.clone(), crate::security::require_token));
    Router::new()
        .merge(prompts::router())
        .merge(sources::router())
        .merge(stats::router())
        .merge(fields::router())
        .merge(settings::router())
        .merge(data::router())
        .merge(security::router())
        .merge(stories::router())
        .merge(export::router())
        .merge(status::router())
        .merge(writes)
        .merge(ingest)
        .with_state(service)
}
