// Copyright (C) 2021-2025  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Runner for the library [`spigot_visual`]

use tiny_http::{Header, Response};

use clap::Parser as _;
use spigot_visual::{
    HTTP_CODE_301_MOVED, HTTP_CODE_404_NOT_FOUND,
    app::{SpigotCommand, app_logic},
    static_file,
    websocket::WebsocketUpgrade,
};
use std::net::SocketAddr;

#[derive(clap::Parser)]
struct Config {
    #[clap(long, default_value = "127.0.0.1:8080")]
    bind_address: SocketAddr,

    #[clap(long)]
    dev_path_prefix: Option<String>,
}

fn main() -> eyre::Result<()> {
    let config = Config::parse();

    let server = match tiny_http::Server::http(config.bind_address) {
        Ok(server) => server,
        Err(e) => eyre::bail!(e),
    };

    let (cmd_tx, cmd_rx) = std::sync::mpsc::sync_channel(1);

    eprintln!("Listening on {}...", config.bind_address);
    std::thread::spawn(move || {
        let server_result = run_server(&server, &config, &cmd_tx);
        match server_result {
            Ok(never) => match never {},
            Err(e) => cmd_tx.send(Err(e)),
        }
    });

    app_logic(cmd_rx)
}

fn run_server(
    server: &tiny_http::Server,
    config: &Config,
    cmd_tx: &std::sync::mpsc::SyncSender<eyre::Result<SpigotCommand>>,
) -> eyre::Result<std::convert::Infallible> {
    loop {
        let request = match server.recv() {
            Ok(request) => request,
            Err(e) => {
                eyre::bail!(e)
            }
        };

        match handle_request(request, config, cmd_tx) {
            Ok(()) => {}
            Err(e) => {
                let e = eyre::eyre!(e);
                eprintln!("failed to handle request: {e:?}");
            }
        }
    }
}

fn handle_request(
    request: tiny_http::Request,
    config: &Config,
    cmd_tx: &std::sync::mpsc::SyncSender<eyre::Result<SpigotCommand>>,
) -> eyre::Result<()> {
    const INDEX: &str = "/index.html";

    // Check if this is a WebSocket upgrade request
    if request.url() == "/ws"
        && let Some(ws_upgrade) = WebsocketUpgrade::new(&request)
    {
        ws_upgrade.spawn(request, cmd_tx.clone());
        return Ok(());
    }

    let prefix = config.dev_path_prefix.as_deref();

    let url = request.url();
    match url {
        s if s == INDEX => static_file!("index.html").reply_html(request, prefix),
        "/app.js" => static_file!("app.js").reply_js(request, prefix),
        "/app.css" => static_file!("app.css").reply_css(request, prefix),
        "/sample-input.js" => static_file!("sample-input.js").reply_js(request, prefix),
        "/van-1.5.5.js" => static_file!("van-1.5.5.js").reply_js(request, prefix),
        "/" => {
            let redirect = Response::empty(HTTP_CODE_301_MOVED)
                .with_header(Header::from_bytes("Location", INDEX).expect("valid header"));
            request.respond(redirect)?;
            Ok(())
        }
        _ => {
            request.respond(Response::empty(HTTP_CODE_404_NOT_FOUND))?;
            Ok(())
        }
    }
}
