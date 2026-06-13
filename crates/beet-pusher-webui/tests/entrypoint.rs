#![expect(missing_docs, reason = "TODO while building")]

use axum::{Router, response::Response};
use beet_pusher_webui::create_app;

mod common {
    mod api_happy_paths;
}

async fn test_app() -> Router {
    let config = beet_pusher_webui::config::Config { port: 0 };
    create_app(config).await
}

async fn response_json(response: Response) -> eyre::Result<serde_json::Value> {
    const LIMIT: usize = 1024 * 1024;
    let body = axum::body::to_bytes(response.into_body(), LIMIT).await?;
    let json = serde_json::from_slice(&body[..])?;
    Ok(json)
}
