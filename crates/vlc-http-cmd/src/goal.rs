// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//
//! High-level command types for VLC

use crate::url::Url;

/// High-level desired state for VLC (dynamic API calls depending on the current state), with no output.
/// (think `Result<(), Error>`)
///
/// See also: [`Command`](`crate::Command`)s for simple changes that do not rely on the current
/// client state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Goal {
    /// Set the item selection mode
    PlaybackMode(PlaybackMode),
    /// Set the current playing and up-next playlist URLs, clearing the history to the specified max count
    PlaylistSet(TargetPlaylistItems),
}
impl From<PlaybackMode> for Goal {
    fn from(value: PlaybackMode) -> Self {
        Self::PlaybackMode(value)
    }
}
impl From<TargetPlaylistItems> for Goal {
    fn from(value: TargetPlaylistItems) -> Self {
        Self::PlaylistSet(value)
    }
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
    #[expect(missing_docs, reason = "self-explanatory")]
    pub const fn get_repeat(self) -> RepeatMode {
        self.repeat
    }
    #[expect(missing_docs, reason = "self-explanatory")]
    #[must_use]
    pub const fn is_random(self) -> bool {
        self.is_random
    }
    #[expect(missing_docs, reason = "self-explanatory")]
    #[must_use]
    pub fn is_loop_all(self) -> bool {
        self.repeat == RepeatMode::All
    }
    #[expect(missing_docs, reason = "self-explanatory")]
    #[must_use]
    pub fn is_repeat_one(self) -> bool {
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

/// Target parameters for [`Goal::PlaylistSet`]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[must_use]
pub struct TargetPlaylistItems {
    urls: Vec<Url>,
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
    pub fn set_urls(mut self, urls: Vec<Url>) -> Self {
        self.urls = urls;
        self
    }
    /// Set the number of history (past-played) items to retain before the specified [`Url`]s
    pub fn set_keep_history(mut self, keep_items: u16) -> Self {
        self.max_history_count = keep_items;
        self
    }
    /// Returns the inner parts of the target
    #[must_use]
    pub fn into_parts(self) -> TargetPlaylistItemsIntoParts {
        let Self {
            urls,
            max_history_count,
        } = self;
        TargetPlaylistItemsIntoParts {
            urls,
            max_history_count,
        }
    }
    /// Returns the [`Url`]s, see [`Self::set_urls`] for semantic details
    #[must_use]
    pub fn get_urls(&self) -> &[Url] {
        &self.urls
    }
}
/// Exposes all inner fields of [`TargetPlaylistItems`]
///
/// NOTE: This intermediate step is intended to disallow "commanders" from directly accessing
/// fields, whereas the "command executor" (`vlc-http`) is allowed to exhaustively use fields
pub struct TargetPlaylistItemsIntoParts {
    /// [`Url`]s to queue next, starting with the current/past item
    pub urls: Vec<Url>,
    /// Number of history (past-played) items to retain before the specified [`Url`]s
    pub max_history_count: u16,
}
