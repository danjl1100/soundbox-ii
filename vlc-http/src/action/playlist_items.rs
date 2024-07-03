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
//! - Precedence of operations:
//!     - Remove (1) - Before adding new, remove the history (decrease the search space)
//!     - Remove (5) - Ensure newly-added items will be continuous with matched items
//!     - Add (4) - Add new desired items to the end
//!     - Remove (3) - Remove items blocking the desired items (all in place)
//!     - NOTE: No action performed for (2), application will send `Command::SeekNext` when immediate
//!     playback is required
//! - Key Constraints:
//!     - "Remove(5)" comes before "Add (4)" for consistent matches
//!     - "Remove(3)" comes after "Add(4)" for seamless playback progression to desired items
//!     - (minor) "Remove (1)" can go anywhere, but place first as a "pre-step" before material
//!     changes to the playback order
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
//! - All sections populated, with some 4-5 mixing
//!
//!     ```text
//!     | X1 | X2 | X3 | P | X4 | X5 | X6 | M1 | X7 | M2 | M3 | X8 | X9 |
//!     |<-----(1)---->|(2)|<----(3)----->|-(4)|(5)-|<--(4)-->|<--(5)-->|
//!     ```
//!

use super::{
    playback_mode, query_playback::QueryPlayback, query_playlist::QueryPlaylist, Error, Poll,
    PollableConstructor,
};
use crate::{action::PlaybackMode, fmt::DebugUrl, Command, Pollable};

mod insert_match;
mod next_command;

#[derive(Debug)]
pub(crate) struct Set {
    target: Target<crate::fmt::DebugUrl>,
    playback_mode: playback_mode::Set,
    query_playback: QueryPlayback,
    query_playlist: QueryPlaylist,
}
#[derive(Debug)]
pub(crate) struct Target<T> {
    /// Path to the file(s) to queue next, starting with the current/past item
    ///
    /// NOTE: When an item is already playing, the first element in `urls` is only matched **at** or
    /// **after** the currently playing item
    pub urls: Vec<T>,
    /// Number of history (past-played) items to retain before the specified `urls`
    pub max_history_count: u16,
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
            playlist
                .iter()
                .position(|item| playing_item_id == item.get_id())
        });

        if let Some(command) = self.target.next_command(playlist, playing_item_index) {
            let command = match command {
                next_command::NextCommand::PlaylistAdd(url) => {
                    Command::PlaylistAdd { url: url.0.clone() }
                }
                next_command::NextCommand::PlaylistDelete(item) => Command::PlaylistDelete {
                    item_id: item.get_id(),
                },
            };
            Ok(Poll::Need(command.into()))
        } else {
            Ok(Poll::Done(()))
        }
    }
}

impl PollableConstructor for Set {
    type Args = Target<url::Url>;
    fn new(target: Self::Args, state: &crate::ClientState) -> Self {
        const LINEAR_PLAYBACK: PlaybackMode = PlaybackMode::new()
            .set_repeat(crate::action::RepeatMode::Off)
            .set_random(false);
        let target = {
            let Target {
                urls,
                max_history_count,
            } = target;
            Target {
                urls: urls.into_iter().map(DebugUrl).collect(),
                max_history_count,
            }
        };
        Self {
            target,
            playback_mode: playback_mode::Set::new(LINEAR_PLAYBACK, state),
            query_playback: QueryPlayback::new((), state),
            query_playlist: QueryPlaylist::new((), state),
        }
    }
}
