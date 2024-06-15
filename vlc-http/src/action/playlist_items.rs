// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

//! Sets the playlist to the specified items
//!
//! ### Implementation notes
//!
//! Labeling sections in the playlist:
//!
//! 1. History items (to be kept, per `max_history_count`)
//! 2. Current playing item
//! 3. Queued items to be deleted (before the `match_start`)
//! 4. Matched items to keep
//! 5. Queued items to be deleted (after the `match_start`)
//!
//! Notes:
//!
//! - (2) and (3) are empty if nothing is playing
//! - (2) and (3) are empty if the first matched item is playing
//! - (4) and (5) are empty if no items match
//! - Precedence of removing items:
//!     - (5) Ensure newly-added items will be continuous with matched items
//!     - (1) Before adding new, remove the history (decrease the search space)
//!
//! Examples:
//!
//! - Empty, after some matched are added
//!
//!     ```text
//!     | M1 | M2 |
//!     |<--(4)-->|
//!     ```
//!
//! - None playing
//!
//!     ```text
//!     | X1 | X2 | X3 | X4 | X5 | M1 | M2 | M3 | X7 | X8 |
//!     |<---------(1)---------->|<----(4)----->|<--(5)-->|
//!     ```
//!
//! - Playing is Matched (P=M1)
//!
//!     ```text
//!     | X1 | X2 | X3 | P = M1 | X4 | X5 |
//!     |<----(1)----->|<-(4)-->|<--(5)-->|
//!     ```
//!
//! - All sections populated
//!
//!     ```text
//!     | X1 | X2 | X3 | P | X4 | X5 | X6 | M1 | M2 | M3 | X7 | X8 |
//!     |<-----(1)---->|(2)|<----(3)----->|<----(4)----->|<--(5)-->|
//!     ```
//!

use super::{
    playback_mode, query_playback::QueryPlayback, query_playlist::QueryPlaylist, Error, Poll,
    PollableConstructor,
};
use crate::{action::PlaybackMode, Pollable};

mod insert_match;
mod next_command;

#[derive(Debug)]
pub(crate) struct Set {
    target: Target,
    playback_mode: playback_mode::Set,
    query_playback: QueryPlayback,
    query_playlist: QueryPlaylist,
}
#[derive(Debug)]
pub(crate) struct Target {
    /// NOTE: The first element of `urls` is accepted as previously-played if it is the most recent history item.
    pub urls: Vec<url::Url>,
    pub max_history_count: std::num::NonZeroU16,
}

impl Pollable for Set {
    type Output<'a> = ();

    fn next(&mut self, state: &crate::ClientState) -> Result<Poll<()>, Error> {
        match self.playback_mode.next(state)? {
            Poll::Done(()) => {}
            Poll::Need(endpoint) => return Ok(Poll::Need(endpoint)),
        }

        let playback = match self.query_playback.next(state)? {
            Poll::Done(playback) => playback,
            Poll::Need(endpoint) => return Ok(Poll::Need(endpoint)),
        };

        let playlist = match self.query_playlist.next(state)? {
            Poll::Done(playlist) => playlist,
            Poll::Need(endpoint) => return Ok(Poll::Need(endpoint)),
        };

        let playing_item_id = playback
            .information
            .as_ref()
            .and_then(|info| info.playlist_item_id);

        let playing_item_index = playing_item_id.and_then(|playing_item_id| {
            playlist.iter().position(|item| playing_item_id == item.id)
        });

        if let Some(command) = self.target.next_command(playlist, playing_item_index) {
            Ok(Poll::Need(command.into()))
        } else {
            Ok(Poll::Done(()))
        }
    }
}

impl PollableConstructor for Set {
    type Args = Target;
    fn new(target: Self::Args, state: &crate::ClientState) -> Self {
        const LINEAR_PLAYBACK: PlaybackMode = PlaybackMode::new()
            .set_repeat(crate::action::RepeatMode::Off)
            .set_random(false);
        Self {
            target,
            playback_mode: playback_mode::Set::new(LINEAR_PLAYBACK, state),
            query_playback: QueryPlayback::new((), state),
            query_playlist: QueryPlaylist::new((), state),
        }
    }
}
