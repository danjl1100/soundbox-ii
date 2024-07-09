// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//
//! High-level actions for VLC (correspond to a single API call)

use crate::{
    client_state::{ClientStateRef, ClientStateSequence, InvalidClientInstance, Sequence},
    response, ClientState, Endpoint,
};

mod playback_mode;
mod playlist_items;

mod query_playback;
mod query_playlist;

/// High-level actions to control VLC (dynamic API calls depending on the current state)
///
/// NOTE: The enum variants are for non-query actions (think `Result<(), Error>`)
///
/// See the inherent functions for queries with specific return results
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Set the item selection mode
    PlaybackMode(PlaybackMode),
    /// Set the current playing and up-next playlist URLs, clearing the history to the specified max count
    ///
    /// See also: [`Action::set_playlist_query_matched`] for obtaining the list of matched items
    PlaylistSet(TargetPlaylistItems),
}
/// Rule for selecting the next playback item in the VLC queue
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub struct PlaybackMode {
    repeat: RepeatMode,
    is_random: bool,
}
impl Default for PlaybackMode {
    fn default() -> Self {
        Self::new()
    }
}
impl PlaybackMode {
    /// Creates the default playback mode
    pub const fn new() -> Self {
        Self {
            repeat: RepeatMode::Off,
            is_random: false,
        }
    }
    /// Sets the VLC playback repeat strategy
    pub const fn set_repeat(mut self, repeat: RepeatMode) -> Self {
        self.repeat = repeat;
        self
    }
    /// Randomizes the VLC playback order when `true`
    pub const fn set_random(mut self, is_random: bool) -> Self {
        self.is_random = is_random;
        self
    }
    #[allow(missing_docs)] // self-explanatory
    pub const fn get_repeat(self) -> RepeatMode {
        self.repeat
    }
    #[allow(missing_docs)] // self-explanatory
    #[must_use]
    pub const fn is_random(self) -> bool {
        self.is_random
    }
    fn is_loop_all(self) -> bool {
        self.repeat == RepeatMode::All
    }
    fn is_repeat_one(self) -> bool {
        self.repeat == RepeatMode::One
    }
}

/// Rule for repeating items
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[must_use]
pub enum RepeatMode {
    /// Stop the VLC queue after playing all items
    #[default]
    Off,
    /// Repeat the VLC queue after playing all items
    All,
    /// Repeat only the current item
    One,
}

/// Target parameters for [`Action::PlaylistSet`]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[must_use]
pub struct TargetPlaylistItems {
    urls: Vec<url::Url>,
    max_history_count: u16,
}
impl TargetPlaylistItems {
    /// Constructs the default target, no items and removing all history items from the playlist
    pub fn new() -> Self {
        Self::default()
    }
    /// Set the path to the file(s) to queue next, starting with the current/past item
    ///
    /// NOTE: When an item is already playing, the first element in `urls` is only matched **at** or
    /// **after** the currently playing item
    pub fn set_urls(mut self, urls: Vec<url::Url>) -> Self {
        self.urls = urls;
        self
    }
    /// Set the number of history (past-played) items to retain before the specified `urls`
    pub fn set_keep_history(mut self, keep_items: u16) -> Self {
        self.max_history_count = keep_items;
        self
    }
}

/// [`Pollable`] container for various (non-query) [`Action`]s
#[derive(Clone, Debug)]
enum ActionPollableInner {
    PlaybackMode(playback_mode::Set),
    PlaylistSet(playlist_items::Set),
}

/// [`Pollable`] container for various (non-query) [`Action`]s
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Debug)]
pub struct ActionPollable(ActionPollableInner);

/// [`Pollable`] container for [`Action::set_playlist_query_matched`]
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Debug)]
pub struct ActionQuerySetItems(playlist_items::Update);

impl Action {
    /// Returns an endpoint source for querying the playlist info
    #[must_use]
    pub fn query_playlist<'a>(
        state: ClientStateRef<'_>,
    ) -> impl Pollable<Output<'a> = &'a [response::playlist::Item]> + 'static {
        query_playlist::QueryPlaylist::new((), state.get_sequence())
    }
    /// Returns an endpoint source for querying the playlist info
    #[must_use]
    pub fn query_playback<'a>(
        state: ClientStateRef<'_>,
    ) -> impl Pollable<Output<'a> = &'a response::PlaybackStatus> + 'static {
        query_playback::QueryPlayback::new((), state.get_sequence())
    }
    /// Returns an endpoint source for setting the `playlist_items` and querying matched items after
    /// the current playing item.
    ///
    /// Output items will be items from a subset of the original target if playing desired items.
    /// The intended use is to advance a "want to play" list based on playback progress.
    #[must_use]
    pub fn set_playlist_query_matched(
        target: TargetPlaylistItems,
        state: ClientStateRef<'_>,
    ) -> ActionQuerySetItems {
        let inner = playlist_items::Update::new(target, state.get_sequence());
        ActionQuerySetItems(inner)
    }
    /// Converts the action into a [`Pollable`] with empty output
    #[must_use]
    pub fn pollable(self, state: ClientStateRef<'_>) -> ActionPollable {
        use ActionPollableInner as Inner;
        let inner = match self {
            Action::PlaybackMode(mode) => {
                Inner::PlaybackMode(playback_mode::Set::new(mode, state.get_sequence()))
            }
            Action::PlaylistSet(target) => {
                Inner::PlaylistSet(playlist_items::Set::new(target, state.get_sequence()))
            }
        };
        ActionPollable(inner)
    }
}

/// Result of [`Pollable`] [`Action`]s
#[derive(Debug, serde::Serialize, PartialEq, Eq)]
pub enum Poll<T> {
    /// Final success output
    Done(T),
    /// Nexxt endpoint required to determine the result
    Need(Endpoint),
}
impl<T> Poll<T> {
    fn map<U>(self, map_fn: impl FnOnce(T) -> U) -> Poll<U> {
        match self {
            Poll::Done(value) => Poll::Done(map_fn(value)),
            Poll::Need(endpoint) => Poll::Need(endpoint),
        }
    }
}
/// Error of [`Pollable`] [`Action`]s
#[derive(Debug, PartialEq, serde::Serialize)]
pub enum Error {
    /// The [`ClientState`] identity changed between creation and poll
    #[allow(missing_docs)]
    InvalidClientInstance(#[serde(skip)] InvalidClientInstance),
}
impl std::error::Error for Error {}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::InvalidClientInstance(_) => {
                write!(f, "action shared among multiple client instances")
            }
        }
    }
}

impl Sequence {
    fn is_after(self, other: Self) -> Result<bool, Error> {
        if let Some(order) = self.try_cmp(&other) {
            // `self.after(other)`: self > other
            Ok(order == std::cmp::Ordering::Greater)
        } else {
            Err(Error::InvalidClientInstance(InvalidClientInstance {
                expected: self,
                found: other,
            }))
        }
    }
}

/// Sequence of endpoints required to calculated the output
pub trait Pollable: std::fmt::Debug {
    /// Final output when no more endpoints are needed
    type Output<'a>: std::fmt::Debug;

    /// Returns an [`Endpoint`] to make progress on the action on the [`ClientState`]
    ///
    /// # Errors
    /// Returns an error describing why no further actions are possible.
    ///
    /// The error may contain a query result (for queries), or an error (for non-queries)
    fn next<'a>(&mut self, state: &'a ClientState) -> Result<Poll<Self::Output<'a>>, Error>;
}
trait PollableConstructor: Pollable
where
    // NOTE: `Serialize` is for tests, hopefully not too invasive?... KEEP THIS TRAIT PRIVATE!
    for<'a> Self::Output<'a>: serde::Serialize,
{
    type Args;
    fn new(args: Self::Args, state: ClientStateSequence) -> Self;
}

impl Pollable for ActionPollable {
    type Output<'a> = ();
    // NOTE: However unlikely it is to mutate `self`, the uniqueness of `self` aligns with usage
    fn next<'a>(&mut self, state: &'a ClientState) -> Result<Poll<Self::Output<'a>>, Error> {
        let Self(inner) = self;
        match inner {
            ActionPollableInner::PlaybackMode(inner) => inner.next(state),
            ActionPollableInner::PlaylistSet(inner) => inner.next(state),
        }
    }
}
impl Pollable for ActionQuerySetItems {
    type Output<'a> = &'a [response::playlist::Item];
    fn next<'a>(&mut self, state: &'a ClientState) -> Result<Poll<Self::Output<'a>>, Error> {
        self.0.next(state)
    }
}
