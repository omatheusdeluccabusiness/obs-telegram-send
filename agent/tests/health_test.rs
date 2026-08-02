use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use obs_telegram_agent::app_with_secret;
use serde_json::Value;
use tower::ServiceExt;

async fn body_json(response: axum::response::Response) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn health_returns_the_current_protocol_version() {
    let response = app_with_secret("test-install-bearer")
        .oneshot(Request::get("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        body_json(response).await,
        serde_json::json!({"version": "0.1.0", "status": "ok"})
    );
}

#[tokio::test]
async fn dynamic_job_routes_reach_the_handlers() {
    for (method, uri) in [
        ("GET", "/v1/jobs/missing-job"),
        ("POST", "/v1/jobs/missing-job/retry"),
    ] {
        let response = app_with_secret("test-install-bearer")
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(uri)
                    .header("authorization", "Bearer test-install-bearer")
                    .header("content-type", "application/json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(body_json(response).await["code"], "job_not_found");
    }
}
