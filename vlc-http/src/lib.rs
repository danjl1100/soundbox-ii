// soundbox-ii/vlc-http VLC communication library *don't keep your sounds boxed up*
// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.
//
//! Reads state and sends commands to HTTP interface of VLC

// teach me
#![deny(clippy::pedantic)]
#![allow(clippy::bool_to_int_with_if)] // except this confusing pattern
// no unsafe
#![forbid(unsafe_code)]
// no unwrap
#![deny(clippy::unwrap_used)]
// no panic
#![deny(clippy::panic)]
// docs!
#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

pub use auth::{Authorization, Credentials};
pub mod auth;

pub use controller::Controller;
pub mod controller;

use command::LowCommand;
pub use command::{PublicCommand as Command, RepeatMode};
mod command;

/// Command interface for setting playlist items
pub mod cmd_playlist_items;

mod request;

mod rules;

mod http_client;

pub use vlc_responses::{PlaybackStatus, PlaylistInfo};
pub mod vlc_responses;

pub use action::{Action, IntoAction, ResultReceiver, ResultSender};
mod action {
    use tokio::sync::oneshot;

    use crate::{vlc_responses::UrlsFmt, Command, Error, PlaybackStatus, PlaylistInfo};

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

    /// Action available to be [`run()`](`crate::Controller::run`), with `Sender<T>` for returning the result
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
    #[allow(clippy::module_name_repetitions)]
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
                Self::Command(command, _) => write!(f, "{:?}", CommandFmt(command)),
                Self::QueryPlaybackStatus(_) => write!(f, "QueryPlaybackStatus"),
                Self::QueryPlaylistInfo(_) => write!(f, "QueryPlaylistInfo"),
                Self::QueryArt(_) => write!(f, "QueryArt"),
            }
        }
    }
    impl std::cmp::PartialEq for Action {
        fn eq(&self, rhs: &Self) -> bool {
            ActionType::from(self) == ActionType::from(rhs)
        }
    }
    /// Debug-coercion for `PlaylistAdd` urls to be literal strings
    struct CommandFmt<'a>(&'a Command);
    impl<'a> std::fmt::Debug for CommandFmt<'a> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self.0 {
                Command::PlaylistSet {
                    urls,
                    max_history_count,
                } => f
                    .debug_struct("PlaylistSet")
                    .field("urls", &UrlsFmt(urls))
                    .field("max_history_count", max_history_count)
                    .finish(),
                Command::PlaylistAdd { url } => f
                    .debug_struct("PlaylistAdd")
                    .field("url", &url.to_string())
                    .finish(),
                inner => write!(f, "{inner:?}"),
            }
        }
    }

    #[derive(Clone, Copy, PartialEq)]
    enum ActionType {
        Command,
        QueryPlaybackStatus,
        QueryPlaylistInfo,
        QueryArt,
    }
    impl From<&Action> for ActionType {
        fn from(action: &Action) -> Self {
            match action {
                Action::Command(..) => Self::Command,
                Action::QueryPlaybackStatus(..) => Self::QueryPlaybackStatus,
                Action::QueryPlaylistInfo(..) => Self::QueryPlaylistInfo,
                Action::QueryArt(..) => Self::QueryArt,
            }
        }
    }
}

shared::wrapper_enum! {
    /// Error from the `run()` function
    #[derive(Debug)]
    pub enum Error {
        /// Hyper client-side error
        Hyper(hyper::Error),
        /// Deserialization error
        Serde(serde_json::Error),
        { impl None for }
        /// Invalid URL
        InvalidUrl(String),
        /// Logic error
        Logic(String),
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Hyper(e) => write!(f, "vlc_http hyper error: {e}"),
            Self::Serde(e) => write!(f, "vlc_http serde error: {e}"),
            Self::InvalidUrl(e) => write!(f, "vlc_http invalid-url: {e}"),
            Self::Logic(e) => write!(f, "vlc_http logic error: {e}"),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Hyper(e) => Some(e),
            Self::Serde(e) => Some(e),
            Self::InvalidUrl(..) | Self::Logic(..) => None,
        }
    }
}
