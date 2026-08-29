// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Types to track the state of a specific VLC instance

use self::sequenced::Sequenced;
use crate::{Response, response};

pub(crate) use sequenced::Sequence;
mod sequenced;

/// Tracks the state of a specific VLC instance
#[derive(Clone, Debug)]
#[must_use]
pub struct ClientState {
    // NOTE: All mutable access to state must flow through [`Action`](crate::Action) to ensure the
    // user considered the cache invalidation cases
    pub(crate) mut(self) playlist_info: Sequenced<response::PlaylistInfo>,
    pub(crate) mut(self) playback_status: Sequenced<Option<response::PlaybackStatus>>,
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
                let _ = self.playback_status.replace(Some(*new));
            }
        }
    }

    /// Borrows the [`ClientState`] for constructing a [`Plan`](`super::Plan`)
    ///
    /// The reference is needed to ensure any cached data used in building the
    /// [`Plan`](`super::Plan`)
    /// is not invalidated by a later [`ClientState::update`]
    pub fn build_plan(&self) -> PlanBuilder<'_> {
        self.build_plan_unchecked()
    }
    /// Captures a thin snapshot of the [`ClientState`] for the sole purpose of constructing
    /// [`Plan`]s at a later time that use only cached data (E.g. queries that immediately
    /// return a result)
    ///
    /// <div class="warning">
    /// WARNING: The builder from this function generates plans that can blindly
    /// use stale data. Query plans from this builder are free to return cached
    /// data, without performing any real query.
    /// </div>
    ///
    /// See the recommended [`build_plan()`](`Self::build_plan`) which ensures that new
    /// data is fetched for each plan.
    ///
    /// [`Plan`]: `crate::Plan`
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
            playlist_info: playlist_info.sequence,
            playback_status: playback_status.sequence,
        }
    }
}
impl Default for ClientState {
    fn default() -> Self {
        Self::new()
    }
}

/// View of a [`ClientState`] for use in creating [`Plan`]s
///
/// Created by [`ClientState::build_plan`]
///
/// NOTE: [`Plan`]s depend on the [`ClientState`] to determine when to use cached data or
/// request fresh data. See [`crate::goal`] module for details and related functions.
///
/// NOTE: This struct is intended to be short-lived, created right when needed to create an action
///
/// [`Plan`]: `crate::Plan`
#[derive(Clone, Copy)]
#[must_use]
pub struct PlanBuilder<'a> {
    // NOTE: This artificial lifetime constrains users to guide them to keep short-lived refs
    _phantom: std::marker::PhantomData<&'a ()>,
    pub(crate) mut(self) sequence: ClientStateSequence,
}

/// Instant in the lifetime of the [`ClientState`] cache, for use in
/// [`PlanBuilder::assume_cache_valid_since()`]
#[derive(Clone, Copy, Debug)]
pub(crate) struct ClientStateSequence {
    pub(crate) mut(self) playlist_info: Sequence,
    pub(crate) mut(self) playback_status: Sequence,
}
impl ClientStateSequence {
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
