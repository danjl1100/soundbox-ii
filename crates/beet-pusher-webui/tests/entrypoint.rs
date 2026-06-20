// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
#![expect(missing_docs, reason = "TODO while building")]

use axum::{Router, response::Response};
use beet_pusher_webui::{create_app, init_tracing};

mod common {
    mod api_errors;
    mod api_happy_paths;
    mod end_to_end;
}

static TRACING_ONCE: std::sync::Once = std::sync::Once::new();

async fn test_app() -> Router {
    TRACING_ONCE.call_once(|| {
        init_tracing("DEBUG");
    });

    let config = beet_pusher_webui::config::Config { port: 0 };
    create_app(config).await
}

struct ReadResponse {
    status: axum::http::StatusCode,
    body: axum::body::Bytes,
    json: eyre::Result<serde_json::Value>,
}
async fn read_response(uri: &str, response: Response) -> eyre::Result<ReadResponse> {
    const LIMIT: usize = 1024 * 1024;
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), LIMIT).await?;
    let json = serde_json::from_slice(&body[..]).map_err(Into::into);

    let resp = ReadResponse { status, body, json };
    tracing::debug!(?uri, ?resp);

    Ok(resp)
}
impl std::fmt::Debug for ReadResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { status, body, json } = self;

        let mut d = f.debug_struct("ReadResponse");
        d.field("status", status);

        match json {
            Ok(json) => {
                d.field("json", json);
            }
            Err(_) => {
                d.field("body", &String::from_utf8_lossy(body));
            }
        }

        d.finish()
    }
}
