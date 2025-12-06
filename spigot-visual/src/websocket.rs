//! Framework for upgrading HTTP connections to websocket connections ([`WebsocketUpgrade`]), and running specific
//! [`Command`]s each returning a response.

use crate::HTTP_CODE_101_SWITCHING_PROTOCOLS;
use eyre::Context as _;
use std::marker::PhantomData;
use tiny_http::{Header, Response};
use tungstenite::Utf8Bytes;

/// Context for handling a single websocket connection
#[must_use]
pub struct WebsocketUpgrade<T> {
    ws_key: String,
    _marker: PhantomData<T>,
}
impl<T: Command> WebsocketUpgrade<T> {
    /// Checks if a request is a valid WebSocket upgrade request and returns the key if valid
    #[must_use]
    pub fn new(request: &tiny_http::Request) -> Option<Self> {
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
        Some(Self {
            ws_key,
            _marker: PhantomData,
        })
    }

    /// Spawns a server to handle WebSocket connections
    #[allow(clippy::must_use_candidate)]
    pub fn spawn(
        self,
        request: tiny_http::Request,
        cmd_tx: std::sync::mpsc::SyncSender<T>,
    ) -> std::thread::JoinHandle<()> {
        let Self { ws_key, _marker } = self;

        eprintln!("WebSocket connection initiated");

        // Compute the Sec-WebSocket-Accept value using tungstenite's helper
        let ws_accept = tungstenite::handshake::derive_accept_key(ws_key.as_bytes());

        // Build the WebSocket handshake response with proper headers
        let response = {
            #[allow(clippy::missing_panics_doc)]
            let (h1, h2, h3) = (
                Header::from_bytes("Upgrade", "websocket").expect("valid hard-coded header"),
                Header::from_bytes("Connection", "Upgrade").expect("valid hard-coded header"),
                Header::from_bytes("Sec-WebSocket-Accept", ws_accept.as_str())
                    .expect("valid header from tungstenite"),
            );
            Response::empty(HTTP_CODE_101_SWITCHING_PROTOCOLS)
                .with_header(h1)
                .with_header(h2)
                .with_header(h3)
        };

        // Upgrade the HTTP connection to a WebSocket
        let stream = request.upgrade("websocket", response);

        // Spawn a thread to handle this WebSocket connection
        // This allows the main server loop to continue accepting new connections
        std::thread::spawn(move || {
            let result = WebsocketUpgrade::handle_connection(stream, &cmd_tx);
            eprintln!("WebSocket connection thread ending");
            match result {
                Ok(()) => {}
                Err(e) => {
                    eprintln!("{e:?}");
                }
            }
        })
    }
}

/// Constructor for a command that produces a response
pub trait Command: Send + Sync + Sized + 'static {
    /// Inner command payload type
    type Inner: serde::de::DeserializeOwned;
    /// Response from a command
    type Response: serde::Serialize;
    /// Creates the command
    fn new(inner: Self::Inner) -> (Self, std::sync::mpsc::Receiver<Self::Response>)
    where
        Self: Sized;
}
impl<T: Command> WebsocketUpgrade<T> {
    /// Handles a WebSocket connection in a separate thread
    ///
    /// # Errors
    /// Disconnects the websocket and returns an error if the communication channel or de/serialization fails
    pub fn handle_connection(
        stream: Box<dyn tiny_http::ReadWrite + Send>,
        cmd_tx: &std::sync::mpsc::SyncSender<T>,
    ) -> eyre::Result<()> {
        use tungstenite::{Message, WebSocket};

        // Wrap the stream in a WebSocket without performing another handshake
        let mut websocket =
            WebSocket::from_raw_socket(stream, tungstenite::protocol::Role::Server, None);

        eprintln!("WebSocket handshake completed");

        // Handle messages until `Close` or an error occurs
        loop {
            let msg = websocket.read().context("WebSocket read error")?;
            match msg {
                Message::Text(text) => {
                    eprintln!("Received: {text}");

                    let kind: T::Inner = serde_json::from_str(&text)
                        .with_context(|| format!("failed to deserialize: {text}"))?;

                    let (cmd, response_rx) = T::new(kind);
                    cmd_tx.send(cmd).context("failed to send logic command")?;

                    let response = response_rx
                        .recv()
                        .context("failed to wait for logic response")?;

                    websocket
                        .write(Message::Text(Utf8Bytes::from(
                            serde_json::to_string(&response)
                                .context("failed to serialize json response")?,
                        )))
                        .context("failed to send websocket response")?;
                    eprintln!("Sent echo response");
                    // Flush to ensure the message is sent
                    if let Err(e) = websocket.flush() {
                        eprintln!("Error flushing websocket: {e}");
                    }
                }
                Message::Binary(data) => {
                    eyre::bail!("unimplemented: Message::Binary({data:?})")
                    // eprintln!("Received binary data: {} bytes", data.len());
                    // if let Err(e) = websocket.write(Message::Binary(data)) {
                    //     eprintln!("Error sending binary data: {e}");
                    //     break;
                    // }
                }
                Message::Ping(data) => {
                    websocket
                        .write(Message::Pong(data))
                        .context("failed to send pong in response to ping")?;
                }
                Message::Pong(_) | Message::Frame(_) => {
                    // Ignore pong messages and raw frames
                }
                Message::Close(_) => {
                    eyre::bail!("WebSocket connection closed by client")
                }
            }
        }
    }
}
