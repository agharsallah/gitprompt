//! The prompt list, its live stream, one prompt, and the user's edits (port of
//! `api/routes/prompts.py`).

use std::convert::Infallible;

use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use tokio_stream::Stream;
use tokio_stream::wrappers::ReceiverStream;

use gitprompt_insights::notify::Notification;

use super::extract::{Body, Params, Prompts};
use crate::bus::Notice;
use crate::error::ApiResult;
use crate::model::{Annotation, PromptDetail, PromptOut, Updated};
use crate::service::Service;

pub(super) fn router() -> Router<Service> {
    Router::new().route("/api/prompts", get(list)).route("/api/prompts/stream", get(stream)).route("/api/prompts/{id}", get(one))
}

pub(super) fn writes() -> Router<Service> {
    Router::new().route("/api/prompts/{id}/annotation", put(annotate)).route("/api/prompts/{id}/redact", post(redact))
}

async fn list(State(service): State<Service>, Prompts(query): Prompts) -> ApiResult<Json<Vec<PromptOut>>> {
    service.prompts(query).await.map(Json)
}

async fn one(State(service): State<Service>, Params(id): Params<i64>) -> ApiResult<Json<PromptDetail>> {
    service.prompt(id).await.map(Json)
}

async fn annotate(State(service): State<Service>, Params(id): Params<i64>, Body(note): Body<Annotation>) -> ApiResult<Json<Annotation>> {
    service.annotate(id, note).await.map(Json)
}

async fn redact(State(service): State<Service>, Params(id): Params<i64>) -> ApiResult<Json<Updated>> {
    service.redact(id).await.map(Json)
}

/// `data: {"prompts": n, "events": m}` per capture, and `event: notification` with a
/// [`Notification`] as `data` per notification the daemon decided on, until the client goes or the
/// server stops.
async fn stream(State(service): State<Service>) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    // Subscribed before the response is returned.
    let (mut notices, mut notifications) = (service.bus().subscribe(), service.bus().notifications());
    let closing = service.closing();
    let (tx, rx) = tokio::sync::mpsc::channel(16);
    tokio::spawn(async move {
        loop {
            let event = tokio::select! {
                () = closing.cancelled() => break,
                () = tx.closed() => break,
                notice = notices.next() => notice.map(notice_event),
                notification = notifications.next() => notification.as_ref().map(notification_event),
            };
            let Some(event) = event else { break };
            if tx.send(Ok(event)).await.is_err() {
                break;
            }
        }
    });
    Sse::new(ReceiverStream::new(rx)).keep_alive(KeepAlive::default())
}

fn notice_event(notice: Notice) -> Event {
    Event::default().data(format!(r#"{{"prompts":{},"events":{}}}"#, notice.prompts, notice.events))
}

fn notification_event(notification: &Notification) -> Event {
    // A plain struct of strings and enums: it always serializes.
    Event::default().event("notification").data(serde_json::to_string(notification).unwrap_or_default())
}
