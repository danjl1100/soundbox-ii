// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! [`clap`] compatible versions of types

use crate::command::VolumeBoundsError;
// avoid local name conflicts
use crate::request::AuthInput as CrateAuthInput;
use crate::Command as CrateCommand;

// re-export `clap`
#[allow(clippy::module_name_repetitions)]
pub use ::clap as clap_crate;

/// Low-level Control commands for VLC (correspond to a single API call)
#[derive(Clone, clap::Subcommand, Debug)]
#[non_exhaustive]
pub enum Command {
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
impl TryFrom<Command> for CrateCommand {
    type Error = VolumeBoundsError;
    fn try_from(value: Command) -> Result<Self, VolumeBoundsError> {
        use CrateCommand as Dest;
        Ok(match value {
            Command::PlaylistAdd { url } => Dest::PlaylistAdd { url },
            Command::PlaylistDelete { item_id } => Dest::PlaylistDelete { item_id },
            Command::PlaylistPlay { item_id } => Dest::PlaylistPlay { item_id },
            Command::ToggleRandom => Dest::ToggleRandom,
            Command::ToggleRepeatOne => Dest::ToggleRepeatOne,
            Command::ToggleLoopAll => Dest::ToggleLoopAll,
            Command::PlaybackResume => Dest::PlaybackResume,
            Command::PlaybackPause => Dest::PlaybackPause,
            Command::PlaybackStop => Dest::PlaybackStop,
            Command::SeekNext => Dest::SeekNext,
            Command::SeekPrevious => Dest::SeekPrevious,
            Command::SeekTo { seconds } => Dest::SeekTo { seconds },
            Command::SeekRelative { seconds_delta } => Dest::SeekRelative {
                seconds_delta: seconds_delta.into(),
            },
            Command::Volume { percent } => Dest::Volume {
                percent: percent.try_into()?,
            },
            Command::VolumeRelative { percent_delta } => Dest::VolumeRelative {
                percent_delta: percent_delta.try_into()?,
            },
            Command::PlaybackSpeed { speed } => Dest::PlaybackSpeed { speed },
        })
    }
}

/// Input authentication parameters to the VLC instance
#[derive(Clone, clap::Args, Debug)]
pub struct AuthInput {
    /// Password string (plaintext)
    #[clap(long, env = "VLC_PASSWORD")]
    pub password: String,
    /// Host string
    #[clap(long, env = "VLC_HOST")]
    pub host: String,
    /// Port number
    #[clap(long, env = "VLC_PORT")]
    pub port: u16,
}
impl From<AuthInput> for CrateAuthInput {
    fn from(value: AuthInput) -> Self {
        let AuthInput {
            password,
            host,
            port,
        } = value;
        Self {
            password,
            host,
            port,
        }
    }
}

/// High-level actions to control VLC (dynamic API calls depending on the current state)
#[derive(Clone, clap::Subcommand, Debug)]
#[non_exhaustive]
pub enum Action {
    /// Set the item selection mode
    PlaybackMode {
        /// Rule for repeating items
        repeat_mode: RepeatMode,
        /// Randomize the VLC playback order
        #[clap(long)]
        random: bool,
    },
    /// Set the current playing and up-next playlist URLs, clearing the history to the specified max count
    ///
    /// See also: [`PlaylistSetQueryMatched`] for obtaining the list of matched items
    PlaylistSet(PlaylistSetQueryMatched),
}
/// Rule for repeating items
#[derive(clap::ValueEnum, Debug, Clone, Copy)]
#[must_use]
pub enum RepeatMode {
    /// Stop the VLC queue after playing all items
    RepeatOff,
    /// Repeat the VLC queue after playing all items
    RepeatAll,
    /// Repeat only the current item
    RepeatOne,
}
impl From<RepeatMode> for crate::action::RepeatMode {
    fn from(value: RepeatMode) -> Self {
        match value {
            RepeatMode::RepeatOff => Self::Off,
            RepeatMode::RepeatAll => Self::All,
            RepeatMode::RepeatOne => Self::One,
        }
    }
}
impl From<Action> for crate::Action {
    fn from(value: Action) -> Self {
        match value {
            Action::PlaybackMode {
                repeat_mode,
                random,
            } => {
                let mode = crate::action::PlaybackMode::default()
                    .set_repeat(repeat_mode.into())
                    .set_random(random);
                Self::PlaybackMode(mode)
            }
            Action::PlaylistSet(target) => Self::PlaylistSet(target.into()),
        }
    }
}

/// Target for a playlist set action/query
#[derive(clap::Args, Clone, Debug)]
pub struct PlaylistSetQueryMatched {
    /// Path to the file(s) to queue next, starting with the current/past item
    urls: Vec<url::Url>,
    /// Minimum number of history (past-played) items to retain
    #[clap(long, default_value_t = 10)]
    keep_history: u16,
}
impl From<PlaylistSetQueryMatched> for crate::action::TargetPlaylistItems {
    fn from(value: PlaylistSetQueryMatched) -> Self {
        let PlaylistSetQueryMatched { urls, keep_history } = value;
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
