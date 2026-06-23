// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! HTTP server frontend to drive [`beet_pusher`]

use beet_pusher_webui::{config::Config, create_app, init_tracing};

/// Signal to shutdown the application
struct Shutdown;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    init_tracing("INFO");

    tracing::info!("Startup");

    let (shutdown_tx, shutdown_rx) = tokio::sync::mpsc::channel(1);
    std::thread::spawn(move || {
        for _line in std::io::stdin().lines() {
            // TODO
        }
        let _ = shutdown_tx.blocking_send(Shutdown);
    });

    let config = Config::from_env()?;

    let app = create_app(config.clone()).await;

    let addr = format!("0.0.0.0:{}", config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    let addr = listener.local_addr()?;
    tracing::info!(%addr, "Listening for HTTP connections");

    if let Ok(port_path) = std::env::var("SCRIPT_WRITE_PORT") {
        std::thread::spawn(move || {
            let contents = format!("{}", addr.port());
            let result = std::fs::write(port_path, &contents);

            if let Err(e) = result {
                let err = eyre::eyre!(e);
                tracing::error!(?err, "failed to write port file");
            }
        });
    }

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(shutdown_rx))
        .await?;

    Ok(())
}

async fn shutdown_signal(mut shutdown_rx: tokio::sync::mpsc::Receiver<Shutdown>) {
    shutdown_rx.recv().await;
}
