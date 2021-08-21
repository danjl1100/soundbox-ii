//! Reads state and sends commands to HTTP interface of VLC

// TODO: only while building
#![allow(dead_code)]
// teach me
#![deny(clippy::pedantic)]
// no unsafe
#![forbid(unsafe_code)]
// no unwrap
#![deny(clippy::unwrap_used)]
// no panic
#![deny(clippy::panic)]
// docs!
#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

use tokio::sync::{mpsc, oneshot};

/// Sender for an action result
pub type ResultSender<T> = oneshot::Sender<Result<T, Error>>;
/// Receiver for an action result
pub type ResultReceiver<T> = oneshot::Receiver<Result<T, Error>>;

/// Action available to be `run()`, with `Sender<T>` for returning the result
#[must_use]
#[derive(Debug)]
pub enum Action {
    /// [`Command`] with `Sender` for the result
    Command(Command, ResultSender<()>),
    /// [`Query`] with a `Sender` for the result
    QueryPlaybackStatus(ResultSender<PlaybackStatus>),
    /// [`Query`] with a `Sender` for the result
    QueryPlaylistInfo(ResultSender<PlaylistInfo>),
}
impl Action {
    /// Constructs a [`Query::PlaybackStatus`] action variant, with the corresponding
    /// [`ResultReceiver`]
    pub fn query_playback_status() -> (Self, ResultReceiver<PlaybackStatus>) {
        let (result_tx, result_rx) = oneshot::channel();
        let action = Self::QueryPlaybackStatus(result_tx);
        (action, result_rx)
    }
    /// Constructs a [`Query::PlaylistInfo`] action variant, with the corresponding
    /// [`ResultReceiver`]
    pub fn query_playlist_info() -> (Self, ResultReceiver<PlaylistInfo>) {
        let (result_tx, result_rx) = oneshot::channel();
        let action = Self::QueryPlaylistInfo(result_tx);
        (action, result_rx)
    }
}
/// Type that can transform into an [`Action`] and [`ResultReceiver`] pair
pub trait IntoAction {
    /// Type output by the [`ResultReceiver`]
    type Output;
    /// Converts the object to an [`Action`] and [`ResultReceiver`] pair
    fn to_action_rx(self) -> (Action, ResultReceiver<Self::Output>);
}
impl IntoAction for Command {
    type Output = ();
    fn to_action_rx(self) -> (Action, ResultReceiver<()>) {
        let (result_tx, result_rx) = oneshot::channel();
        let action = Action::Command(self, result_tx);
        (action, result_rx)
    }
}
impl std::fmt::Display for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Command(command, _) => write!(f, "{:?}", command),
            Self::QueryPlaybackStatus(_) => write!(f, "{:?}", Query::PlaybackStatus),
            Self::QueryPlaylistInfo(_) => write!(f, "{:?}", Query::PlaylistInfo),
        }
    }
}

pub use command::{Command, Query};
mod command;

mod request;

pub use vlc_responses::{PlaybackStatus, PlaylistInfo};
pub mod vlc_responses;

pub(crate) use context::Context;
mod context {
    use super::{auth::Credentials, command::RequestIntent, request::RequestInfo};
    use hyper::{
        body::Body, client::Builder as ClientBuilder, Client as HyperClient,
        Request as HyperRequest,
    };
    type Client = HyperClient<hyper::client::connect::HttpConnector, Body>;
    type Request = HyperRequest<Body>;

    /// Execution context for [`RequestIntent`]s
    pub(crate) struct Context(Client, Credentials);
    impl Context {
        pub fn new(credentials: Credentials) -> Self {
            let client = ClientBuilder::default().build_http();
            Self(client, credentials)
        }
        pub async fn run<'a, 'b>(
            &self,
            request_intent: &RequestIntent<'a, 'b>,
        ) -> Result<hyper::Response<Body>, hyper::Error> {
            let request_info = RequestInfo::from(request_intent);
            Ok(self.run_retry_loop(request_info).await?)
        }
        async fn run_retry_loop(
            &self,
            request: RequestInfo,
        ) -> Result<hyper::Response<Body>, hyper::Error> {
            use backoff::{future::retry, ExponentialBackoff};
            use tokio::time::Duration;
            let backoff_config = ExponentialBackoff {
                current_interval: Duration::from_millis(50),
                initial_interval: Duration::from_millis(50),
                multiplier: 4.0,
                max_interval: Duration::from_secs(2),
                max_elapsed_time: Some(Duration::from_secs(10)),
                ..ExponentialBackoff::default()
            };
            retry(backoff_config, || async {
                let request = self.request_from(request.clone()); //TODO: avoid expensive clone?
                Ok(self.0.request(request).await?)
            })
            .await
        }
        fn request_from(&self, info: RequestInfo) -> Request {
            let RequestInfo {
                path_and_query,
                method,
            } = info;
            let uri = self
                .1
                .uri_builder()
                .path_and_query(path_and_query)
                .build()
                .expect("internally-generated URI is valid");
            self.1
                .request_builder()
                .uri(uri)
                .method(method)
                .body(Body::empty())
                .expect("internally-generated URI and Method is valid")
        }
    }
}

pub use auth::{Config, Credentials};
pub mod auth;

/// Error from the `run()` function
#[derive(Debug)]
pub enum Error {
    /// Hyper client-side error
    Hyper(hyper::Error),
    /// Deserialization error
    Serde(serde_json::Error),
}
impl From<hyper::Error> for Error {
    fn from(err: hyper::Error) -> Self {
        Self::Hyper(err)
    }
}
impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Self::Serde(err)
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Hyper(e) => write!(f, "vlc_http hyper error: {}", e),
            Self::Serde(e) => write!(f, "vlc_http serde error: {}", e),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Hyper(e) => Some(e),
            Self::Serde(e) => Some(e),
        }
    }
}

/// Executes the specified commands
pub async fn run(credentials: Credentials, mut commands: mpsc::Receiver<Action>) {
    let context = Context::new(credentials);
    while let Some(action) = commands.recv().await {
        match action {
            Action::Command(command, result_tx) => {
                let request = command.into();
                let result = context
                    .run(&request)
                    .await
                    .map(|_| ())
                    .map_err(|e| e.into());
                send_result(result, result_tx);
            }
            Action::QueryPlaybackStatus(result_tx) => {
                let request = Query::PlaybackStatus.into();
                let response = context.run(&request).await;
                let result = parse_body_json(response, PlaybackStatus::from_slice).await;
                send_result(result, result_tx);
            }
            Action::QueryPlaylistInfo(result_tx) => {
                let request = Query::PlaylistInfo.into();
                let response = context.run(&request).await;
                let result = parse_body_json(response, PlaylistInfo::from_slice).await;
                send_result(result, result_tx);
            }
        };
    }
    println!("vlc_http::run() - context ended!");
}
async fn parse_body_json<F, T, E>(
    result: Result<hyper::Response<hyper::Body>, hyper::Error>,
    map_fn: F,
) -> Result<T, Error>
where
    F: FnOnce(&[u8]) -> Result<T, E>,
    Error: From<E>,
{
    match result {
        Ok(response) => hyper::body::to_bytes(response.into_body())
            .await
            .map_err(|err| err.into())
            .and_then(|bytes| Ok(map_fn(&bytes)?)),
        Err(err) => Err(err.into()),
    }
}
fn send_result<T>(result: Result<T, Error>, result_tx: ResultSender<T>)
where
    T: std::fmt::Debug,
{
    let send_result = result_tx.send(result);
    if let Err(send_err) = send_result {
        println!("WARNING: result_tx send error: {:?}", send_err);
    }
}
