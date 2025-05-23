// Copyright (C) 2021-2025  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Types to track the state of a specific VLC instance

use self::sequenced::Sequenced;
use crate::{response, Response};

pub(crate) use sequenced::Sequence;
mod sequenced;

/// Tracks the state of a specific VLC instance
#[derive(Clone, Debug)]
#[must_use]
pub struct ClientState {
    playlist_info: Sequenced<response::PlaylistInfo>,
    playback_status: Sequenced<Option<response::PlaybackStatus>>,
}

impl ClientState {
    /// Returns an empty state
    pub fn new() -> Self {
        let builder = Sequenced::builder();
        Self {
            playlist_info: builder.next_default(),
            playback_status: builder.next_default(),
        }
    }

    /// Updates the state for the specified [`Response`]
    ///
    /// This allows [`Plan`](`crate::Plan`)s to progress to return a result, or a new
    /// [`Endpoint`](`crate::Endpoint`)
    pub fn update(&mut self, response: Response) {
        match response.inner {
            crate::response::ResponseInner::PlaylistInfo(new) => {
                let _ = self.playlist_info.replace(new);
            }
            crate::response::ResponseInner::PlaybackStatus(new) => {
                let _ = self.playback_status.replace(Some(new));
            }
        }
    }

    /// Returns a short-lived builder referencing the current [`ClientState`]
    ///
    /// The reference is needed to ensure any cached data used in building the
    /// [`Plan`](`super::Plan`)
    /// is not invalidated by a later [`ClientState::update`]
    pub fn build_plan(&self) -> PlanBuilder<'_> {
        self.build_plan_unchecked()
    }
    /// Returns a builder referencing an old/outdated [`ClientState`]
    ///
    /// <div class="warning">
    /// WARNING: The builder from this function generates plans that can blindly
    /// use stale data. Query plans from this builder are free to return cached
    /// data, without performing any real query.
    /// </div>
    ///
    /// Use [`build_plan()`](`Self::build_plan`) instead, to ensure that new
    /// data is fetched for each plan.
    pub fn assume_cache_valid_for_later_building(&self) -> PlanBuilder<'static> {
        self.build_plan_unchecked()
    }
    fn build_plan_unchecked(&self) -> PlanBuilder<'static> {
        PlanBuilder {
            _phantom: std::marker::PhantomData,
            sequence: self.get_sequence(),
        }
    }
    pub(crate) fn get_sequence(&self) -> ClientStateSequence {
        let Self {
            playlist_info,
            playback_status,
        } = self;
        ClientStateSequence {
            playlist_info: playlist_info.get_sequence(),
            playback_status: playback_status.get_sequence(),
        }
    }

    /// NOTE: All access to state must flow through [`Action`](crate::Action) to ensure the user
    /// considered the cache invalidation cases
    pub(crate) fn playlist_info(&self) -> &Sequenced<response::PlaylistInfo> {
        &self.playlist_info
    }
    /// NOTE: All access to state must flow through [`Action`](crate::Action) to ensure the user
    /// considered the cache invalidation cases
    pub(crate) fn playback_status(&self) -> &Sequenced<Option<response::PlaybackStatus>> {
        &self.playback_status
    }
}
impl Default for ClientState {
    fn default() -> Self {
        Self::new()
    }
}

/// Reference to a [`ClientState`] for use in creating an action
///
/// Created by [`ClientState::build_plan`]
///
/// See [`crate::goal`] and related functions
///
/// NOTE: This struct is intended to be short-lived, created right when needed to create an action
#[derive(Clone, Copy)]
#[must_use]
pub struct PlanBuilder<'a> {
    // NOTE: This artificial lifetime constrains users to guide them to keep short-lived refs
    _phantom: std::marker::PhantomData<&'a ()>,
    sequence: ClientStateSequence,
}
impl PlanBuilder<'_> {
    pub(crate) fn get_sequence(self) -> ClientStateSequence {
        self.sequence
    }
}

/// Instant in the lifetime of the [`ClientState`] cache, for use in
/// [`PlanBuilder::assume_cache_valid_since()`]
#[derive(Clone, Copy, Debug)]
pub(crate) struct ClientStateSequence {
    playlist_info: Sequence,
    playback_status: Sequence,
}
impl ClientStateSequence {
    pub(crate) fn playlist_info(self) -> Sequence {
        self.playlist_info
    }
    pub(crate) fn playback_status(self) -> Sequence {
        self.playback_status
    }
    // fn try_min(self, other: Self) -> Result<Self, InvalidClientInstance> {
    //     let Self {
    //         playlist_info,
    //         playback_status,
    //     } = self;
    //     Ok(Self {
    //         playlist_info: try_min_seq(playlist_info, other.playlist_info)?,
    //         playback_status: try_min_seq(playback_status, other.playback_status)?,
    //     })
    // }
}
// fn try_min_seq(lhs: Sequence, rhs: Sequence) -> Result<Sequence, InvalidClientInstance> {
//     lhs.min(rhs).ok_or(InvalidClientInstance {
//         expected: lhs,
//         found: rhs,
//     })
// }

impl std::fmt::Debug for PlanBuilder<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlanBuilder")
            .field("sequence", &self.sequence)
            .finish()
    }
}

/// Attempt to compare/combine different [`ClientState`]s
#[derive(Debug, PartialEq)]
pub struct InvalidClientInstance {
    pub(crate) expected: Sequence,
    pub(crate) found: Sequence,
}
