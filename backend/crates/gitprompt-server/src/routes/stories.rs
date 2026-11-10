//! Session and project stories, a prompt's versions, and search (port of
//! `api/routes/stories.py`).

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};

use super::extract::{Params, Prompts};
use crate::error::ApiResult;
use crate::model::{Lineage, ProjectStory, SearchResults, SessionStory};
use crate::service::Service;

pub(super) fn router() -> Router<Service> {
    Router::new()
        .route("/api/stories/session/{session}", get(session))
        .route("/api/stories/project/{project}", get(project))
        .route("/api/stories/prompt/{id}", get(lineage))
        .route("/api/search", get(search))
}

async fn session(State(service): State<Service>, Params(session): Params<String>) -> ApiResult<Json<SessionStory>> {
    service.session_story(&session).await.map(Json)
}

async fn project(State(service): State<Service>, Params(project): Params<String>) -> ApiResult<Json<ProjectStory>> {
    service.project_story(&project).await.map(Json)
}

async fn lineage(State(service): State<Service>, Params(id): Params<i64>) -> ApiResult<Json<Lineage>> {
    service.lineage(id).await.map(Json)
}

async fn search(State(service): State<Service>, Prompts(query): Prompts) -> ApiResult<Json<SearchResults>> {
    service.search(query).await.map(Json)
}
