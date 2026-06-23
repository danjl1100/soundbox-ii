// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::json;
use tower::ServiceExt as _;

use crate::{init_test_tracing, read_response, test_app};

#[tokio::test]
async fn health() -> eyre::Result<()> {
    init_test_tracing();
    let app = test_app().await;

    let response = app
        .oneshot(Request::builder().uri("/health").body(Body::empty())?)
        .await?;

    assert_eq!(response.status(), StatusCode::OK);

    Ok(())
}

#[tokio::test]
async fn add_node() -> eyre::Result<()> {
    init_test_tracing();
    let app = test_app().await;

    let uri = "/api/v1/nodes/create-bucket";
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("Content-Type", "application/json")
                .body(
                    json!({
                        "parent": ".",
                    })
                    .to_string(),
                )?,
        )
        .await?;

    let resp = read_response(uri, response).await?;

    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(
        resp.json?,
        json!({
            "status": "success",
            "data": {
                "path": ".0",
            },
        }),
        "uri={uri:?}"
    );

    Ok(())
}
