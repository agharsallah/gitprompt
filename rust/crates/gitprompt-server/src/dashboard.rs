//! The built dashboard (SvelteKit static build), served at `/`. A page path with no file of its own
//! (a story: `/sessions/<id>/`) gets the build's fallback page [`FALLBACK`], which routes in the
//! browser. Never for `/api/**` and `/ingest/**` (they keep their 404), nor for a path whose last
//! segment has a file extension (a missing asset is a 404, not an HTML page).
//! A directory path serves its `index.html`.

use std::path::PathBuf;

use axum::Router;

pub const FALLBACK: &str = "200.html";

/// Whether a request path that matched no file gets [`FALLBACK`].
pub fn serves_fallback(_path: &str) -> bool {
    todo!()
}

/// The static files of `dir` with the fallback rule, as the router's fallback service.
pub fn router(_dir: PathBuf) -> Router {
    todo!()
}
