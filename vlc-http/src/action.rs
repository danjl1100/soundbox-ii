// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//
//! High-level actions for VLC (correspond to a single API call)

use std::num::NonZeroUsize;

/// High-level actions to control VLC (dynamic API calls depending on the current state)
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Set the current playing and up-next playlist URLs, clearing the history to the specified max count
    ///
    /// NOTE: The first element of `urls` is accepted as previously-played if it is the most recent history item.
    /// NOTE: Forces the playback mode to `{ repeat: RepeatMode::Off, random: false }`
    PlaylistSet {
        /// Path to the file(s) to queue next, starting with the current/past item
        urls: Vec<url::Url>,
        /// Maximum number of history (past-played) items to retain
        ///
        /// NOTE: Enforced as non-zero, since at least 1 "history" item is needed to:
        ///  * detect the "past" case of `current_or_past_url`, and
        ///  * add current the playlist (to retain during the 1 tick where current is added, but not yet playing)
        max_history_count: NonZeroUsize,
    },
    /// Set the item selection mode
    PlaybackMode {
        #[allow(missing_docs)]
        repeat: RepeatMode,
        /// Randomizes the VLC playback order when `true`
        random: bool,
    },
}
/// Rule for selecting the next playback item in the VLC queue
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeatMode {
    /// Stop the VLC queue after playing all items
    Off,
    /// Repeat the VLC queue after playing all items
    All,
    /// Repeat only the current item
    One,
}
// impl RepeatMode {
//     pub(crate) fn is_loop_all(self) -> bool {
//         self == Self::All
//     }
//     pub(crate) fn is_repeat_one(self) -> bool {
//         self == Self::One
//     }
// }
