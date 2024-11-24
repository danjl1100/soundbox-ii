// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
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
    /// Returns an empty state for a specific VLC client
    pub fn new() -> Self {
        let builder = Sequenced::builder();
        Self {
            playlist_info: builder.next_default(),
            playback_status: builder.next_default(),
        }
    }

    /// Updates the state for the specified [`Response`]
    ///
    /// Returns the [`ClientStateSequence`] for the previous cache instant (for use in
    /// [`ClientStateRef::assume_cache_valid_since()`].
    ///
    /// This allows [`Action`](`crate::Action`)s to progress to return a result, or a new
    /// [`Endpoint`](`crate::Endpoint`)
    pub fn update(&mut self, response: Response) -> ClientStateSequence {
        let previous_cache_instant = self.get_sequence();
        match response.inner {
            crate::response::ResponseInner::PlaylistInfo(new) => {
                let _prev = self.playlist_info.replace(new);
            }
            crate::response::ResponseInner::PlaybackStatus(new) => {
                let _prev = self.playback_status.replace(Some(new));
            }
        }
        previous_cache_instant
    }

    /// Returns a handle for use in actions
    pub fn get_ref(&self) -> ClientStateRef<'_> {
        ClientStateRef {
            _phantom: std::marker::PhantomData,
            sequence: self.get_sequence(),
        }
    }
    fn get_sequence(&self) -> ClientStateSequence {
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
/// See [`crate::Action::pollable()`] and related functions
///
/// NOTE: This struct is intended to be short-lived, created right when needed to create an action
#[derive(Clone, Copy)]
#[must_use]
#[allow(clippy::module_name_repetitions)]
pub struct ClientStateRef<'a> {
    // NOTE: This artificial lifetime constrains users to guide them to keep short-lived refs
    _phantom: std::marker::PhantomData<&'a ()>,
    sequence: ClientStateSequence,
}
impl ClientStateRef<'_> {
    /// The returned reference will start the [`crate::Action`] as if it was created before the specified
    /// [`ClientStateSequence`] instant.
    ///
    /// # Errors
    /// Returns an error if the specified [`ClientStateSequence`] is from a different instance from
    /// the current [`ClientState`]
    pub fn assume_cache_valid_since(
        mut self,
        other: ClientStateSequence,
    ) -> Result<Self, InvalidClientInstance> {
        self.sequence = self.sequence.try_min(other)?;
        Ok(self)
    }
    pub(crate) fn get_sequence(self) -> ClientStateSequence {
        self.sequence
    }
}

/// Instant in the lifetime of the [`ClientState`] cache, for use in
/// [`ClientStateRef::assume_cache_valid_since()`]
#[derive(Clone, Copy, Debug)]
#[allow(clippy::module_name_repetitions)]
pub struct ClientStateSequence {
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
    fn try_min(self, other: Self) -> Result<Self, InvalidClientInstance> {
        let Self {
            playlist_info,
            playback_status,
        } = self;
        Ok(Self {
            playlist_info: try_min_seq(playlist_info, other.playlist_info)?,
            playback_status: try_min_seq(playback_status, other.playback_status)?,
        })
    }
}
fn try_min_seq(lhs: Sequence, rhs: Sequence) -> Result<Sequence, InvalidClientInstance> {
    lhs.min(rhs).ok_or(InvalidClientInstance {
        expected: lhs,
        found: rhs,
    })
}

impl std::fmt::Debug for ClientStateRef<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientStateRef")
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
