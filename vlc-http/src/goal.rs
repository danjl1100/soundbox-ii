// Copyright (C) 2021-2025  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//
//! High-level actions for VLC, requiring multiple steps to reach the desired state

use crate::{
    client_state::{ClientStateRef, ClientStateSequence, InvalidClientInstance, Sequence},
    response, ClientState, Endpoint,
};

mod playback_mode;
mod playlist_items;

mod query_playback;
mod query_playlist;

mod builders {
    use super::{
        playlist_items, query_playback::QueryPlayback, query_playlist::QueryPlaylist, ActionPlan,
        ActionQuerySetItems, Change, PlanConstructor as _, TargetPlaylistItems,
    };
    use crate::{goal::playback_mode, ClientState};

    // TODO change to `PlanBuilder<'a>` (used to be ClientStateRef<'_>), with functions
    // - `ClientState::build_plan(&self) -> PlanBuilder<'_>` and
    // - `ClientState::assume_cache_valid_for_later_building(&self) -> PlanBuilder<'static>` with documentation
    //   about how this can cause plans (and especially queries) to blindly use stale data.
    //   `build_plan()` is recommended for ensuring that new data is fetched for each plan
    impl ClientState {
        /// Creates a [`Plan`](`super::Plan`) to query the playlist items
        pub fn query_playlist(&self) -> QueryPlaylist {
            QueryPlaylist::new((), self.get_sequence())
        }
        /// Creates a [`Plan`](`super::Plan`) to query the playback status
        pub fn query_playback(&self) -> QueryPlayback {
            QueryPlayback::new((), self.get_sequence())
        }
        /// Returns an endpoint source for setting the `playlist_items` and querying matched items after
        /// the current playing item.
        ///
        /// Output items will be items from a subset of the original target if playing desired items.
        /// The intended use is to advance a "want to play" list based on playback progress.
        pub fn set_playlist_and_query_matched(
            &self,
            target: TargetPlaylistItems,
        ) -> ActionQuerySetItems {
            let inner = playlist_items::Update::new(target, self.get_sequence());
            ActionQuerySetItems(inner)
        }
        /// Creates a [`Plan`](`super::Plan`) to apply the desired change
        pub fn apply(&self, change: Change) -> ActionPlan {
            use super::ActionPlanInner as Inner;
            let inner = match change {
                Change::PlaybackMode(mode) => {
                    Inner::PlaybackMode(playback_mode::Set::new(mode, self.get_sequence()))
                }
                Change::PlaylistSet(target) => {
                    Inner::PlaylistSet(playlist_items::Set::new(target, self.get_sequence()))
                }
            };
            ActionPlan(inner)
        }
    }
}

/// High-level change to VLC state (dynamic API calls depending on the current state), with no output.
/// (think `Result<(), Error>`)
///
/// Used with [`ClientState::apply`] to create a [`Plan`] to execute.
///
/// See also: [`Command`](`crate::Command`)s for simple changes that do not rely on the current
/// client state.
///
/// See also: Query methods on [`ClientState`] for obtain non-empty data results.
///
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// Set the item selection mode
    PlaybackMode(PlaybackMode),
    /// Set the current playing and up-next playlist URLs, clearing the history to the specified max count
    ///
    /// See also: [`Change::set_playlist_query_matched`] for obtaining the list of matched items
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
    #[expect(missing_docs)] // self-explanatory
    pub const fn get_repeat(self) -> RepeatMode {
        self.repeat
    }
    #[expect(missing_docs)] // self-explanatory
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

/// Target parameters for [`Change::PlaylistSet`]
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

#[derive(Clone, Debug)]
enum ActionPlanInner {
    PlaybackMode(playback_mode::Set),
    PlaylistSet(playlist_items::Set),
}

/// [`Plan`] container for various (non-query) [`Change`]s
#[derive(Clone, Debug)]
#[must_use]
pub struct ActionPlan(ActionPlanInner);

/// [`Plan`] container for [`Change::set_playlist_query_matched`]
#[must_use]
#[derive(Clone, Debug)]
pub struct ActionQuerySetItems(playlist_items::Update);

impl Change {
    /// Returns an endpoint source for querying the playlist info
    #[deprecated = "use the ClientState function instead"]
    #[must_use]
    pub fn query_playlist<'a>(
        state: ClientStateRef<'_>,
    ) -> impl Plan<Output<'a> = &'a [response::playlist::Item]> + 'static {
        query_playlist::QueryPlaylist::new((), state.get_sequence())
    }
    /// Returns an endpoint source for querying the playlist info
    #[deprecated = "use the ClientState function instead"]
    #[must_use]
    pub fn query_playback<'a>(
        state: ClientStateRef<'_>,
    ) -> impl Plan<Output<'a> = &'a response::PlaybackStatus> + 'static {
        query_playback::QueryPlayback::new((), state.get_sequence())
    }
    /// Returns an endpoint source for setting the `playlist_items` and querying matched items after
    /// the current playing item.
    ///
    /// Output items will be items from a subset of the original target if playing desired items.
    /// The intended use is to advance a "want to play" list based on playback progress.
    #[deprecated = "use the ClientState function instead"]
    pub fn set_playlist_query_matched(
        target: TargetPlaylistItems,
        state: ClientStateRef<'_>,
    ) -> ActionQuerySetItems {
        let inner = playlist_items::Update::new(target, state.get_sequence());
        ActionQuerySetItems(inner)
    }
    /// Converts the action into a [`Plan`] with empty output
    #[deprecated = "use the ClientState function instead"]
    pub fn into_plan(self, state: ClientStateRef<'_>) -> ActionPlan {
        use ActionPlanInner as Inner;
        let inner = match self {
            Change::PlaybackMode(mode) => {
                Inner::PlaybackMode(playback_mode::Set::new(mode, state.get_sequence()))
            }
            Change::PlaylistSet(target) => {
                Inner::PlaylistSet(playlist_items::Set::new(target, state.get_sequence()))
            }
        };
        ActionPlan(inner)
    }
}

/// Result for one part in reaching a goal
#[derive(Debug, serde::Serialize, PartialEq, Eq)]
pub enum Step<T> {
    /// Final success output
    Done(T),
    /// Nexxt endpoint required to determine the result
    Need(Endpoint),
}
impl<T> Step<T> {
    /// Change the [`Self::Done`] type
    fn map<U>(self, map_fn: impl FnOnce(T) -> U) -> Step<U> {
        match self {
            Step::Done(value) => Step::Done(map_fn(value)),
            Step::Need(endpoint) => Step::Need(endpoint),
        }
    }
    /// Discard the [`Self::Done`] data
    fn ignore_done(self) -> Step<()> {
        self.map(|_| ())
    }
}
/// Error of [`Plan`] [`Change`]s
#[derive(Debug)]
pub enum Error {
    /// The [`ClientState`] identity changed between creation and executing the [`Plan`]
    InvalidClientInstance(InvalidClientInstance),
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

/// Sequence of endpoints required to accomplish the high-level goal
pub trait Plan: std::fmt::Debug {
    /// Final output when no more endpoints are needed
    type Output<'a>: std::fmt::Debug;

    /// Returns an [`Endpoint`] to make progress on the action on the [`ClientState`]
    ///
    /// # Errors
    /// Returns an error describing why no further steps are possible to reach the end goal.
    fn next<'a>(&mut self, state: &'a ClientState) -> Result<Step<Self::Output<'a>>, Error>;
}
trait PlanConstructor: Plan
where
    // NOTE: `Serialize` is for tests, hopefully not too invasive?... KEEP THIS TRAIT PRIVATE!
    for<'a> Self::Output<'a>: serde::Serialize,
{
    type Args;
    fn new(args: Self::Args, state: ClientStateSequence) -> Self;
}

impl Plan for ActionPlan {
    type Output<'a> = ();
    // NOTE: However unlikely it is to mutate `self`, the uniqueness of `self` aligns with usage
    fn next<'a>(&mut self, state: &'a ClientState) -> Result<Step<Self::Output<'a>>, Error> {
        let Self(inner) = self;
        match inner {
            ActionPlanInner::PlaybackMode(inner) => inner.next(state),
            ActionPlanInner::PlaylistSet(inner) => inner.next(state),
        }
    }
}
impl Plan for ActionQuerySetItems {
    type Output<'a> = &'a [response::playlist::Item];
    fn next<'a>(&mut self, state: &'a ClientState) -> Result<Step<Self::Output<'a>>, Error> {
        self.0.next(state)
    }
}
