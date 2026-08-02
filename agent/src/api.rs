use axum::{
    extract::{Request, State},
    http::{header::AUTHORIZATION, HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use serde::Serialize;

use crate::{
    config::{validate_config, ConfigInput},
    keychain::SecretStore,
};

#[derive(Serialize)]
pub struct AgentError {
    pub code: String,
    pub message: String,
}

impl AgentError {
    fn unauthorized() -> Self {
        Self {
            code: "unauthorized".to_owned(),
            message: "A valid install bearer is required.".to_owned(),
        }
    }

    fn invalid_configuration() -> Self {
        Self {
            code: "invalid_configuration".to_owned(),
            message: "The Telegram configuration is invalid.".to_owned(),
        }
    }

    fn storage_unavailable() -> Self {
        Self {
            code: "storage_unavailable".to_owned(),
            message: "Secure configuration storage is unavailable.".to_owned(),
        }
    }
}

pub fn router(install_secret: String) -> Router {
    Router::new()
        .route("/v1/config", post(save_configuration))
        .route_layer(middleware::from_fn_with_state(
            install_secret,
            require_bearer,
        ))
}

async fn require_bearer(State(secret): State<String>, request: Request, next: Next) -> Response {
    let expected = HeaderValue::from_str(&format!("Bearer {secret}"));
    if expected.ok().as_ref() == request.headers().get(AUTHORIZATION) {
        return next.run(request).await;
    }

    (StatusCode::UNAUTHORIZED, Json(AgentError::unauthorized())).into_response()
}

async fn save_configuration(Json(input): Json<ConfigInput>) -> Response {
    let configuration = match validate_config(input) {
        Ok(configuration) => configuration,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(AgentError::invalid_configuration()),
            )
                .into_response()
        }
    };

    let result = SecretStore::new().and_then(|store| store.save(&configuration));
    match result {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AgentError::storage_unavailable()),
        )
            .into_response(),
    }
}
