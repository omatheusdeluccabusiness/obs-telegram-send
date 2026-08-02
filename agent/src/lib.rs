use axum::{routing::get, Json, Router};
use serde_json::json;

pub mod api;
pub mod config;
pub mod install_bearer;
pub mod keychain;

pub fn app_with_secret(install_secret: &str) -> Router {
    Router::new()
        .route(
            "/health",
            get(|| async { Json(json!({"version": "0.1.0", "status": "ok"})) }),
        )
        .merge(api::router(install_secret.to_owned()))
}
