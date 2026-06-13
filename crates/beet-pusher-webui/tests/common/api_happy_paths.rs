use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::json;
use tower::ServiceExt as _;

use crate::{response_json, test_app};

#[tokio::test]
#[ignore = "TODO"]
async fn health() -> eyre::Result<()> {
    let app = test_app().await;

    let response = app
        .oneshot(Request::builder().uri("/health").body(Body::empty())?)
        .await?;

    assert_eq!(response.status(), StatusCode::OK);

    Ok(())
}

#[tokio::test]
#[ignore = "TODO"]
async fn add_node() -> eyre::Result<()> {
    let app = test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/nodes/./create-bucket")
                .body(Body::empty())?,
        )
        .await?;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response_json(response).await?,
        json!({
            "data": {
                "path": ".0 or smth amazing I guess",
            },
        })
    );

    Ok(())
}
