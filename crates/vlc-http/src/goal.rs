// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//
//! High-level actions for VLC, requiring multiple steps to reach the desired state

pub use self::playlist_items::Output as OutputQuerySetItems;
use crate::{
    ClientState, Endpoint,
    client_state::{ClientStateSequence, InvalidClientInstance, Sequence},
    response,
};
pub use vlc_http_cmd::{
    command::{VolumePercent, VolumePercentDelta},
    goal::{Goal, PlaybackMode, RepeatMode, TargetPlaylistItems},
};

mod playback_mode;
mod playlist_items;

mod query_playback;
mod query_playlist;

mod builders {
    use super::{
        ActionPlan, ActionQuerySetItems, Goal, PlanConstructor as _, TargetPlaylistItems,
        playlist_items, query_playback::QueryPlayback, query_playlist::QueryPlaylist,
    };
    use crate::{client_state::PlanBuilder, goal::playback_mode};

    impl PlanBuilder<'_> {
        /// Creates a [`Plan`](`super::Plan`) to query the playlist items
        pub fn query_playlist(self) -> QueryPlaylist {
            QueryPlaylist::new((), self.sequence)
        }
        /// Creates a [`Plan`](`super::Plan`) to query the playback status
        pub fn query_playback(self) -> QueryPlayback {
            QueryPlayback::new((), self.sequence)
        }
        /// Returns an endpoint source for setting the `playlist_items` and querying matched items
        /// after the current playing item.
        ///
        /// Output items will be items from a subset of the original target if playing desired items.
        /// The intended use is to advance a "want to play" list based on playback progress.
        pub fn set_playlist_and_query_matched(
            self,
            target: TargetPlaylistItems,
        ) -> ActionQuerySetItems {
            let inner = playlist_items::Update::new(target, self.sequence);
            ActionQuerySetItems(inner)
        }
        /// Creates a [`Plan`](`super::Plan`) to apply the desired goal
        pub fn apply(self, goal: Goal) -> ActionPlan {
            use super::ActionPlanInner as Inner;
            let inner = match goal {
                Goal::PlaybackMode(mode) => {
                    Inner::PlaybackMode(playback_mode::Set::new(mode, self.sequence))
                }
                Goal::PlaylistSet(target) => {
                    Inner::PlaylistSet(playlist_items::Set::new(target, self.sequence))
                }
            };
            ActionPlan(inner)
        }
    }
}

#[derive(Clone, Debug)]
enum ActionPlanInner {
    PlaybackMode(playback_mode::Set),
    PlaylistSet(playlist_items::Set),
}

/// [`Plan`] container for various (non-query) [`Goal`]s
#[derive(Clone, Debug)]
#[must_use]
pub struct ActionPlan(ActionPlanInner);

/// [`Plan`] container for
/// [`PlanBuilder::set_playlist_and_query_matched`](`crate::client_state::PlanBuilder::set_playlist_and_query_matched`)
#[must_use]
#[derive(Clone, Debug)]
pub struct ActionQuerySetItems(playlist_items::Update);

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
    pub fn map<U>(self, map_fn: impl FnOnce(T) -> U) -> Step<U> {
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
/// Error executing a [`Plan`]
#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
}
#[derive(Debug)]
enum ErrorKind {
    /// The [`ClientState`] identity changed between creation and executing the [`Plan`]
    InvalidClientInstance(InvalidClientInstance),
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            ErrorKind::InvalidClientInstance(_) => None,
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            ErrorKind::InvalidClientInstance(_inner) => {
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
            Err(Error {
                kind: ErrorKind::InvalidClientInstance(InvalidClientInstance {
                    expected: self,
                    found: other,
                }),
            })
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
    type Output<'a> = OutputQuerySetItems<'a>;
    fn next<'a>(&mut self, state: &'a ClientState) -> Result<Step<Self::Output<'a>>, Error> {
        self.0.next(state)
    }
}
