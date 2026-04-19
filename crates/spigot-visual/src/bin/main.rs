// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Runner for the library [`spigot_visual`]

use tiny_http::{Header, Response};

use clap::Parser as _;
use spigot_visual::{
    HTTP_CODE_301_MOVED, HTTP_CODE_404_NOT_FOUND,
    app::{SpigotCommand, app_logic},
    mpsc_channel::{heartbeat_sender, spawn_narrower_sender},
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

#[derive(Debug)]
enum ServerFatalError {
    Recv(std::io::Error),
}
impl std::error::Error for ServerFatalError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ServerFatalError::Recv(source) => Some(source),
        }
    }
}
impl std::fmt::Display for ServerFatalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServerFatalError::Recv(_source) => write!(f, "failed to receive HTTP request"),
        }
    }
}

fn main() -> eyre::Result<()> {
    let config = Config::parse();

    let server = match tiny_http::Server::http(config.bind_address) {
        Ok(server) => server,
        Err(e) => eyre::bail!(e),
    };

    let (cmd_err_tx, cmd_err_rx) = std::sync::mpsc::sync_channel(1);

    eprintln!("Listening on {}...", config.bind_address);
    let (cmd_tx, _handle) = spawn_narrower_sender(cmd_err_tx.clone(), |msg| {
        Ok(spigot_visual::app::MsgOrHeartbeat::Msg(msg))
    });
    std::thread::spawn({
        let cmd_err_tx = cmd_err_tx.clone();
        move || {
            let interval = std::time::Duration::from_secs(1);
            let Err(_) = heartbeat_sender(&cmd_err_tx, interval, || {
                Ok(spigot_visual::app::MsgOrHeartbeat::Heartbeat)
            });
        }
    });
    std::thread::spawn(move || {
        let Err(fatal_error) = run_server(&server, &config, &cmd_tx);
        cmd_err_tx.send(Err(fatal_error))
    });

    app_logic(cmd_err_rx).map_err(|e| match e {
        spigot_visual::app::Error::Server(e) => eyre::eyre!(e),
        spigot_visual::app::Error::App(e) => e,
    })
}

fn run_server(
    server: &tiny_http::Server,
    config: &Config,
    cmd_tx: &std::sync::mpsc::SyncSender<SpigotCommand>,
) -> Result<std::convert::Infallible, ServerFatalError> {
    loop {
        let request = server.recv().map_err(ServerFatalError::Recv)?;

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
    cmd_tx: &std::sync::mpsc::SyncSender<SpigotCommand>,
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
