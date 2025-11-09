// Copyright (C) 2021-2025  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Runner for the library [`spigot_visual`]

use tiny_http::{Header, Response};

use clap::Parser as _;
use spigot_visual::static_file;
use std::net::SocketAddr;

const HTTP_CODE_101_SWITCHING_PROTOCOLS: u32 = 101;
const HTTP_CODE_301_MOVED: u32 = 301;
const HTTP_CODE_404_NOT_FOUND: u32 = 404;

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

    eprintln!("Listening on {}...", config.bind_address);

    loop {
        let request = match server.recv() {
            Ok(request) => request,
            Err(e) => {
                eyre::bail!(e);
            }
        };

        match handle_request(request, &config) {
            Ok(()) => {}
            Err(e) => {
                let e = eyre::eyre!(e);
                eprintln!("failed to handle request: {e:?}");
            }
        }
    }
}

struct WebsocketUpgrade {
    ws_key: String,
}

impl WebsocketUpgrade {
    /// Checks if a request is a valid WebSocket upgrade request and returns the key if valid
    fn new(request: &tiny_http::Request) -> Option<Self> {
        let true = request.headers().iter().any(|h| {
            h.field.equiv("Upgrade") && h.value.as_str().eq_ignore_ascii_case("websocket")
        }) else {
            return None;
        };

        let true = request.headers().iter().any(|h| {
            h.field.equiv("Connection")
                && h.value
                    .as_str()
                    .split(',')
                    .any(|s| s.trim().eq_ignore_ascii_case("Upgrade"))
        }) else {
            return None;
        };

        let ws_key = request
            .headers()
            .iter()
            .find(|h| h.field.equiv("Sec-WebSocket-Key"))
            .map(|h| h.value.as_str().to_string())?;

        // Only return the key if all required headers are present
        Some(Self { ws_key })
    }

    /// Handles WebSocket connections
    fn spawn(self, request: tiny_http::Request) {
        let Self { ws_key } = self;

        eprintln!("WebSocket connection initiated");

        // Compute the Sec-WebSocket-Accept value using tungstenite's helper
        let ws_accept = tungstenite::handshake::derive_accept_key(ws_key.as_bytes());

        // Build the WebSocket handshake response with proper headers
        let response = Response::empty(HTTP_CODE_101_SWITCHING_PROTOCOLS)
            .with_header(Header::from_bytes("Upgrade", "websocket").expect("valid header"))
            .with_header(Header::from_bytes("Connection", "Upgrade").expect("valid header"))
            .with_header(
                Header::from_bytes("Sec-WebSocket-Accept", ws_accept.as_str())
                    .expect("valid header"),
            );

        // Upgrade the HTTP connection to a WebSocket
        let stream = request.upgrade("websocket", response);

        // Spawn a thread to handle this WebSocket connection
        // This allows the main server loop to continue accepting new connections
        std::thread::spawn(move || {
            handle_websocket_connection(stream);
        });
    }
}

/// Handles a WebSocket connection in a separate thread
fn handle_websocket_connection(stream: Box<dyn tiny_http::ReadWrite + Send>) {
    use tungstenite::{Message, WebSocket};

    // Wrap the stream in a WebSocket without performing another handshake
    let mut websocket =
        WebSocket::from_raw_socket(stream, tungstenite::protocol::Role::Server, None);

    eprintln!("WebSocket handshake completed");

    // Simple echo server for proof-of-concept
    loop {
        match websocket.read() {
            Ok(msg) => match msg {
                Message::Text(text) => {
                    eprintln!("Received: {text}");
                    // Echo the message back with a prefix
                    let response = format!("Server echo: {text}");
                    match websocket.write(Message::Text(response.into())) {
                        Ok(()) => {
                            eprintln!("Sent echo response");
                            // Flush to ensure the message is sent
                            if let Err(e) = websocket.flush() {
                                eprintln!("Error flushing websocket: {e}");
                            }
                        }
                        Err(e) => {
                            eprintln!("Error sending echo: {e}");
                            break;
                        }
                    }
                }
                Message::Binary(data) => {
                    eprintln!("Received binary data: {} bytes", data.len());
                    if let Err(e) = websocket.write(Message::Binary(data)) {
                        eprintln!("Error sending binary data: {e}");
                        break;
                    }
                }
                Message::Ping(data) => {
                    if let Err(e) = websocket.write(Message::Pong(data)) {
                        eprintln!("Error sending pong: {e}");
                        break;
                    }
                }
                Message::Pong(_) | Message::Frame(_) => {
                    // Ignore pong messages and raw frames
                }
                Message::Close(_) => {
                    eprintln!("WebSocket connection closed by client");
                    break;
                }
            },
            Err(e) => {
                eprintln!("WebSocket error: {e}");
                break;
            }
        }
    }

    eprintln!("WebSocket connection thread ending");
}

fn handle_request(request: tiny_http::Request, config: &Config) -> eyre::Result<()> {
    const INDEX: &str = "/index.html";

    // Check if this is a WebSocket upgrade request
    if request.url() == "/ws"
        && let Some(ws_upgrade) = WebsocketUpgrade::new(&request)
    {
        ws_upgrade.spawn(request);
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
