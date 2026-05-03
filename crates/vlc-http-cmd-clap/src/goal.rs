// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! [`clap`] compatible versions of [`vlc_http_cmd::goal`] types

use crate::Url;
use vlc_http_cmd::goal::{Goal, PlaybackMode, RepeatMode, TargetPlaylistItems};

/// High-level desired state for VLC (dynamic API calls depending on the current state)
#[derive(Clone, clap::Subcommand, Debug)]
pub enum ClapGoal {
    /// Set the item selection mode
    PlaybackMode(ClapGoalPlaybackMode),
    /// Set the current playing and up-next playlist URLs, clearing the history to the specified max count
    ///
    /// See also: [`ClapPlaylistSetQueryMatched`] for obtaining the list of matched items
    PlaylistSet(ClapPlaylistSetQueryMatched),
}
/// Set the item selection mode
#[derive(Clone, clap::Args, Debug)]
pub struct ClapGoalPlaybackMode {
    /// Rule for repeating items
    repeat_mode: ClapRepeatMode,
    /// Randomize the VLC playback order
    #[clap(long)]
    random: bool,
}

/// Rule for repeating items
#[derive(clap::ValueEnum, Debug, Clone, Copy)]
#[must_use]
#[expect(
    clippy::enum_variant_names,
    reason = "common prefix is useful for positional clap naming"
)]
enum ClapRepeatMode {
    /// Stop the VLC queue after playing all items
    RepeatOff,
    /// Repeat the VLC queue after playing all items
    RepeatAll,
    /// Repeat only the current item
    RepeatOne,
}
impl From<ClapRepeatMode> for RepeatMode {
    fn from(value: ClapRepeatMode) -> Self {
        match value {
            ClapRepeatMode::RepeatOff => Self::Off,
            ClapRepeatMode::RepeatAll => Self::All,
            ClapRepeatMode::RepeatOne => Self::One,
        }
    }
}
impl From<ClapGoal> for Goal {
    fn from(value: ClapGoal) -> Self {
        match value {
            ClapGoal::PlaybackMode(ClapGoalPlaybackMode {
                repeat_mode,
                random,
            }) => {
                let mode = PlaybackMode::default()
                    .set_repeat(repeat_mode.into())
                    .set_random(random);
                Self::PlaybackMode(mode)
            }
            ClapGoal::PlaylistSet(target) => Self::PlaylistSet(target.into()),
        }
    }
}

/// Target for a playlist set goal
#[derive(clap::Args, Clone, Debug)]
pub struct ClapPlaylistSetQueryMatched {
    /// Path to the file(s) to queue next, starting with the current/past item
    urls: Vec<Url>,
    /// Minimum number of history (past-played) items to retain
    #[clap(long, default_value_t = 10)]
    keep_history: u16,
}
impl From<ClapPlaylistSetQueryMatched> for TargetPlaylistItems {
    fn from(value: ClapPlaylistSetQueryMatched) -> Self {
        let ClapPlaylistSetQueryMatched { urls, keep_history } = value;
        Self::new()
            .set_urls(urls) //
            .set_keep_history(keep_history)
    }
}

// TODO how to test derived subcommands?
// #[cfg(test)]
// mod tests {
//     use super::*;
//     use clap::Parser as _;
//
//     #[test]
//     fn clap() {
//         #[derive(clap::Command)]
//         struct C {
//             c: Command,
//         }
//     }
// }
