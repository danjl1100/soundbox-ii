// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! HTTP server frontend to drive [`beet_pusher`]

use beet_pusher_webui::{config::Config, create_app};

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let config = Config::from_env()?;

    let app = create_app(config.clone()).await;

    let addr = format!("0.0.0.0:{}", config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app)
        // TODO
        // .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

// async fn shutdown_signal() {
//     loop {
//         tokio::time::sleep(std::time::Duration::from_secs(1)).await;
//     }
// }
