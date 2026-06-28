// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
#![expect(missing_docs, reason = "TODO while building")]

use axum::{Router, response::Response};
use beet_pusher_webui::{create_app, domain::services::ports::BeetPusherPipe};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt as _, util::SubscriberInitExt as _};

mod common {
    mod api_errors;
    mod api_happy_paths;
    mod end_to_end;
}

#[derive(Default)]
struct TestPipe {
    fake_add_node: bool,
}
impl TestPipe {
    #[expect(clippy::needless_update, reason = "only one field")]
    fn fake_add_node(self, fake_add_node: bool) -> Self {
        Self {
            fake_add_node,
            ..self
        }
    }
}
impl BeetPusherPipe for TestPipe {
    type Error = std::convert::Infallible;
    async fn send(
        &self,
        command: beet_pusher::pipe_exec::Command,
        _timeout: std::time::Duration,
    ) -> Result<beet_pusher::pipe_exec::ResponseData, Self::Error> {
        use beet_pusher::pipe_exec::{Command, ResponseData, SpigotCmd, VlcCmd};
        let resp = match command {
            Command::Spigot(SpigotCmd::AddNode {
                parent: mut path,
                node_kind: _,
            }) if self.fake_add_node => {
                path.push(0);
                ResponseData::NodeAdded { path }
            }
            Command::Spigot(
                SpigotCmd::AddNode {
                    parent: _,
                    node_kind: _,
                }
                | SpigotCmd::SetFilters {
                    path: _,
                    new_filters: _,
                },
            )
            | Command::Vlc(VlcCmd::SeekNext) => ResponseData::PassNoData,
        };
        Ok(resp)
    }
}

async fn test_app() -> Router {
    test_app_with_pipe(|p| p).await
}
async fn test_app_with_pipe(pipe_fn: impl FnOnce(TestPipe) -> TestPipe) -> Router {
    let config = beet_pusher_webui::config::Config { port: 0 };
    let pipe = pipe_fn(TestPipe::default());
    create_app(config, pipe).await
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
                d.field(
                    "json",
                    &std::fmt::from_fn(move |f| {
                        let s = if f.alternate() {
                            serde_json::to_string_pretty(&json)
                        } else {
                            serde_json::to_string(&json)
                        }
                        .unwrap_or_else(|e| format!("<SERDE ERR: {e}>"));
                        write!(f, "{s}")
                    }),
                );
            }
            Err(_) => {
                d.field("body", &String::from_utf8_lossy(body));
            }
        }

        d.finish()
    }
}

pub fn init_test_tracing() {
    static TRACING_ONCE: std::sync::Once = std::sync::Once::new();

    TRACING_ONCE.call_once(|| {
        let env_filter = EnvFilter::try_from_default_env().unwrap_or_default();

        let fmt_layer = tracing_subscriber::fmt::layer().with_test_writer();

        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .init();
    });
}
