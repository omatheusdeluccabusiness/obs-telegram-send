use axum::{routing::get, Json, Router};
use serde_json::json;

pub mod config;
pub mod keychain;

pub fn app() -> Router {
    Router::new().route(
        "/health",
        get(|| async { Json(json!({"version": "0.1.0", "status": "ok"})) }),
    )
}
