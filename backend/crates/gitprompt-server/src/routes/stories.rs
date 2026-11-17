//! Session and project stories, a prompt's versions, and search (port of
//! `api/routes/stories.py`).

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};

use super::extract::{Commits, Params, Prompts};
use crate::error::ApiResult;
use crate::model::{Lineage, ProjectCommits, ProjectStory, SearchResults, SessionStory};
use crate::service::Service;

pub(super) fn router() -> Router<Service> {
    Router::new()
        .route("/api/stories/session/{session}", get(session))
        .route("/api/stories/project/{project}", get(project))
        .route("/api/stories/project/{project}/commits", get(commits))
        .route("/api/stories/prompt/{id}", get(lineage))
        .route("/api/search", get(search))
}

async fn session(State(service): State<Service>, Params(session): Params<String>) -> ApiResult<Json<SessionStory>> {
    service.session_story(&session).await.map(Json)
}

async fn project(State(service): State<Service>, Params(project): Params<String>) -> ApiResult<Json<ProjectStory>> {
    service.project_story(&project).await.map(Json)
}

async fn commits(
    State(service): State<Service>,
    Params(project): Params<String>,
    Commits(query): Commits,
) -> ApiResult<Json<ProjectCommits>> {
    service.project_commits(&project, query).await.map(Json)
}

async fn lineage(State(service): State<Service>, Params(id): Params<i64>) -> ApiResult<Json<Lineage>> {
    service.lineage(id).await.map(Json)
}

async fn search(State(service): State<Service>, Prompts(query): Prompts) -> ApiResult<Json<SearchResults>> {
    service.search(query).await.map(Json)
}
