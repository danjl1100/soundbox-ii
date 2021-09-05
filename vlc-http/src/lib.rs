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

use tokio::sync::oneshot;

pub use auth::{Config, Credentials};
pub mod auth;

pub use controller::Controller;
pub mod controller;

pub use command::Command;
use command::Query;
mod command;

mod request;

mod http_client;

pub use vlc_responses::{PlaybackStatus, PlaylistInfo};
pub mod vlc_responses;

/// Sender for an action result
pub type ResultSender<T> = oneshot::Sender<Result<T, Error>>;
/// Receiver for an action result
pub type ResultReceiver<T> = oneshot::Receiver<Result<T, Error>>;

/// Response for [`Action::QueryArt`].
///
/// # Variants:
/// - `Err(hyper_err)` - internal error
/// - `Ok(Err(message))` - text error from VLC
/// - `Ok(Ok(respnse))` - Art response from VLC
pub type Art = Result<Result<hyper::Response<hyper::Body>, String>, hyper::Error>;

/// Action available to be `run()`, with `Sender<T>` for returning the result
#[must_use]
#[derive(Debug)]
pub enum Action {
    /// [`Command`] with `Sender` for the result
    Command(Command, ResultSender<()>),
    /// [`PlaybackStatus`] query, with a `Sender` for the result
    QueryPlaybackStatus(Option<ResultSender<PlaybackStatus>>),
    /// [`PlaylistInfo`] query, with a `Sender` for the result
    QueryPlaylistInfo(Option<ResultSender<PlaylistInfo>>),
    /// Art query, with a `Sender` for the result
    QueryArt(oneshot::Sender<Art>),
}
impl Action {
    /// Constructs a [`PlaybackStatus`] action variant, with no receiver
    pub fn fetch_playback_status() -> Self {
        Self::QueryPlaybackStatus(None)
    }
    /// Constructs a [`PlaybackStatus`] action variant, with no receiver
    pub fn fetch_playlist_info() -> Self {
        Self::QueryPlaylistInfo(None)
    }
    /// Constructs a [`PlaybackStatus`] action variant, with the corresponding
    /// [`ResultReceiver`]
    pub fn query_playback_status() -> (Self, ResultReceiver<PlaybackStatus>) {
        let (result_tx, result_rx) = oneshot::channel();
        let action = Self::QueryPlaybackStatus(Some(result_tx));
        (action, result_rx)
    }
    /// Constructs a [`PlaylistInfo`] action variant, with the corresponding
    /// [`ResultReceiver`]
    pub fn query_playlist_info() -> (Self, ResultReceiver<PlaylistInfo>) {
        let (result_tx, result_rx) = oneshot::channel();
        let action = Self::QueryPlaylistInfo(Some(result_tx));
        (action, result_rx)
    }
    /// Constructs a Art action variant, with the corresponding
    /// [`ResultReceiver`]
    pub fn query_art() -> (Self, oneshot::Receiver<Art>) {
        let (result_tx, result_rx) = oneshot::channel();
        let action = Self::QueryArt(result_tx);
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
            Self::QueryArt(_) => write!(f, "QueryArt"),
        }
    }
}

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
