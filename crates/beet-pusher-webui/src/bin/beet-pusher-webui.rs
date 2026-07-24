// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! HTTP server frontend to drive [`beet_pusher`]

use beet_pusher_webui::{
    config::Config,
    create_app,
    infra::{Shutdown, stdio_pipe::StdioPipe},
    init_tracing,
};
use eyre::Context as _;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    init_tracing("INFO");

    tracing::info!("Startup");

    let config = Config::from_env()?;

    let (shutdown_tx, shutdown_rx) = tokio::sync::mpsc::channel(1);

    let (pipe, stdout_thread, stdin_thread) = StdioPipe::spawn(shutdown_tx);
    let app = create_app(config.clone(), pipe).await;

    let addr = format!("{}:{}", config.bind_ip, config.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .with_context(|| format!("failed to bind TCP listener to {addr}"))?;

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

    stdout_thread.join().expect("panic in pipe thread")?;
    stdin_thread.join().expect("panic in pipe thread")?;

    Ok(())
}

async fn shutdown_signal(mut shutdown_rx: tokio::sync::mpsc::Receiver<Shutdown>) {
    shutdown_rx.recv().await;
}
