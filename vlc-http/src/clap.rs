// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! [`clap`] compatible versions of types

use crate::command::VolumeBoundsError;
// avoid local name conflicts
use crate::Command as CrateCommand;

// re-export `clap`
#[expect(clippy::module_name_repetitions, reason = "re-export `clap`")]
pub use ::clap as clap_crate;

/// Low-level Control commands for VLC (correspond to a single API call)
#[derive(Clone, clap::Args, Debug)]
#[group(skip)]
pub struct ClapCommand {
    #[clap(subcommand)]
    subcommand: ClapCommandInner,
}
#[derive(Clone, clap::Subcommand, Debug)]
enum ClapCommandInner {
    /// Add the specified item to the playlist
    PlaylistAdd {
        /// URL of the file to enqueue (for local files: `file:///path/to/file`)
        url: url::Url,
    },
    /// Deletes the specified item from the playlist
    PlaylistDelete {
        /// Identifier of the playlist item to remove
        item_id: u64,
    },
    /// Play the specified item in the playlist
    PlaylistPlay {
        /// Identifier of the playlist item
        item_id: Option<u64>,
    },
    /// Randomizes VLC playback order when toggled to `true`
    ToggleRandom,
    /// Repeats one VLC item when toggled to `true`
    ToggleRepeatOne,
    /// Repeats the VLC playlist when toggled to `true`
    ToggleLoopAll,
    // ========================================
    /// Force playback to resume
    PlaybackResume,
    /// Force playback to pause
    PlaybackPause,
    /// Force playback to stop, deselecting the current playing item
    PlaybackStop,
    /// Seek to the next item
    SeekNext,
    /// Seek to the previous item
    SeekPrevious,
    /// Seek absolutely within the current item
    SeekTo {
        /// Seconds within the current item
        seconds: u32,
    },
    /// Seek relatively within the current item
    SeekRelative {
        /// Seconds delta within the current item
        seconds_delta: i32,
    },
    /// Set the playback volume
    Volume {
        /// Percentage for the volume (clamped at 300, which means 300% volume)
        percent: u16,
    },
    /// Adjust the playback volume
    VolumeRelative {
        /// Percentage delta for the volume
        percent_delta: i16,
    },
    /// Set the playback speed
    PlaybackSpeed {
        /// Speed on unit scale (1.0 = normal speed)
        speed: f64,
    },
}
impl TryFrom<ClapCommand> for CrateCommand {
    type Error = VolumeBoundsError;
    fn try_from(value: ClapCommand) -> Result<Self, VolumeBoundsError> {
        use ClapCommandInner as Src;
        use CrateCommand as Dest;

        let ClapCommand { subcommand } = value;
        Ok(match subcommand {
            Src::PlaylistAdd { url } => Dest::PlaylistAdd { url },
            Src::PlaylistDelete { item_id } => Dest::PlaylistDelete { item_id },
            Src::PlaylistPlay { item_id } => Dest::PlaylistPlay { item_id },
            Src::ToggleRandom => Dest::ToggleRandom,
            Src::ToggleRepeatOne => Dest::ToggleRepeatOne,
            Src::ToggleLoopAll => Dest::ToggleLoopAll,
            Src::PlaybackResume => Dest::PlaybackResume,
            Src::PlaybackPause => Dest::PlaybackPause,
            Src::PlaybackStop => Dest::PlaybackStop,
            Src::SeekNext => Dest::SeekNext,
            Src::SeekPrevious => Dest::SeekPrevious,
            Src::SeekTo { seconds } => Dest::SeekTo { seconds },
            Src::SeekRelative { seconds_delta } => Dest::SeekRelative {
                seconds_delta: seconds_delta.into(),
            },
            Src::Volume { percent } => Dest::Volume {
                percent: percent.try_into()?,
            },
            Src::VolumeRelative { percent_delta } => Dest::VolumeRelative {
                percent_delta: percent_delta.try_into()?,
            },
            Src::PlaybackSpeed { speed } => Dest::PlaybackSpeed { speed },
        })
    }
}

/// High-level change to VLC state (dynamic API calls depending on the current state)
#[derive(Clone, clap::Subcommand, Debug)]
#[non_exhaustive]
pub enum ClapChange {
    /// Set the item selection mode
    PlaybackMode(ClapChangePlaybackMode),
    /// Set the current playing and up-next playlist URLs, clearing the history to the specified max count
    ///
    /// See also: [`ClapPlaylistSetQueryMatched`] for obtaining the list of matched items
    PlaylistSet(ClapPlaylistSetQueryMatched),
}
/// Set the item selection mode
#[derive(Clone, clap::Args, Debug)]
pub struct ClapChangePlaybackMode {
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
impl From<ClapRepeatMode> for crate::goal::RepeatMode {
    fn from(value: ClapRepeatMode) -> Self {
        match value {
            ClapRepeatMode::RepeatOff => Self::Off,
            ClapRepeatMode::RepeatAll => Self::All,
            ClapRepeatMode::RepeatOne => Self::One,
        }
    }
}
impl From<ClapChange> for crate::Change {
    fn from(value: ClapChange) -> Self {
        match value {
            ClapChange::PlaybackMode(ClapChangePlaybackMode {
                repeat_mode,
                random,
            }) => {
                let mode = crate::goal::PlaybackMode::default()
                    .set_repeat(repeat_mode.into())
                    .set_random(random);
                Self::PlaybackMode(mode)
            }
            ClapChange::PlaylistSet(target) => Self::PlaylistSet(target.into()),
        }
    }
}

/// Target for a playlist set goal
#[derive(clap::Args, Clone, Debug)]
pub struct ClapPlaylistSetQueryMatched {
    /// Path to the file(s) to queue next, starting with the current/past item
    urls: Vec<url::Url>,
    /// Minimum number of history (past-played) items to retain
    #[clap(long, default_value_t = 10)]
    keep_history: u16,
}
impl From<ClapPlaylistSetQueryMatched> for crate::goal::TargetPlaylistItems {
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
