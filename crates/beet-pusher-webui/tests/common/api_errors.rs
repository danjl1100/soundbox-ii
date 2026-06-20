use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt as _;

use crate::{read_response, test_app};

#[tokio::test]
async fn error_not_found() -> eyre::Result<()> {
    let app = test_app().await;

    let uri = "/not/valid/uri";
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .body(Body::empty())?,
        )
        .await?;

    let resp = read_response(uri, response).await?;

    assert_eq!(resp.status, StatusCode::NOT_FOUND);
    // NOTE: I would have preferred an error here, but `axum` sends no body
    assert!(
        resp.body.is_empty(),
        "nonempty {:?}",
        String::from_utf8_lossy(&resp.body)
    );

    Ok(())
}

#[tokio::test]
async fn error_invalid_json() -> eyre::Result<()> {
    let app = test_app().await;

    let uri = "/api/v1/nodes/create-bucket";
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .body(Body::empty())?,
        )
        .await?;

    let resp = read_response(uri, response).await?;

    assert_eq!(resp.status, StatusCode::BAD_REQUEST);
    insta::assert_json_snapshot!(resp.json?, @r#"
    {
      "error": {
        "message": "invalid JSON: Expected request with `Content-Type: application/json`",
        "type": "validation_error"
      },
      "status": "fail"
    }
    "#);

    Ok(())
}

#[tokio::test]
#[ignore = "need an API endpoint with a `validator` error"]
async fn error_invalid_field() -> eyre::Result<()> {
    unimplemented!("API endpoint with a `validator` error (not just `serde`)")

    // let app = test_app().await;
    //
    // let uri = "/api/v1/TODO";
    // let response = app
    //     .oneshot(
    //         Request::builder()
    //             .method("POST")
    //             .uri(uri)
    //             .header("Content-Type", "application/json")
    //             .body(
    //                 json!({
    //                     "parent": ".a",
    //                 })
    //                 .to_string(),
    //             )?,
    //     )
    //     .await?;

    // let resp = read_response(response).await?;
    // tracing::debug!(?uri, ?resp);

    // assert_eq!(resp.status, StatusCode::BAD_REQUEST);
    // insta::assert_json_snapshot!(
    //     resp.json?,
    //     @""
    // );

    // Ok(())
}
