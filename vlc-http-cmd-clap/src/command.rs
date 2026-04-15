// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! [`clap`] compatible versions of [`vlc_http_cmd::command`] types

use crate::Url;
use vlc_http_cmd::command::{Command, VolumeBoundsError};

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
        url: Url,
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
impl TryFrom<ClapCommand> for Command {
    type Error = VolumeBoundsError;
    fn try_from(value: ClapCommand) -> Result<Self, VolumeBoundsError> {
        use ClapCommandInner as Src;

        let ClapCommand { subcommand } = value;
        Ok(match subcommand {
            Src::PlaylistAdd { url } => Self::PlaylistAdd { url },
            Src::PlaylistDelete { item_id } => Self::PlaylistDelete { item_id },
            Src::PlaylistPlay { item_id } => Self::PlaylistPlay { item_id },
            Src::ToggleRandom => Self::ToggleRandom,
            Src::ToggleRepeatOne => Self::ToggleRepeatOne,
            Src::ToggleLoopAll => Self::ToggleLoopAll,
            Src::PlaybackResume => Self::PlaybackResume,
            Src::PlaybackPause => Self::PlaybackPause,
            Src::PlaybackStop => Self::PlaybackStop,
            Src::SeekNext => Self::SeekNext,
            Src::SeekPrevious => Self::SeekPrevious,
            Src::SeekTo { seconds } => Self::SeekTo { seconds },
            Src::SeekRelative { seconds_delta } => Self::SeekRelative {
                seconds_delta: seconds_delta.into(),
            },
            Src::Volume { percent } => Self::Volume {
                percent: percent.try_into()?,
            },
            Src::VolumeRelative { percent_delta } => Self::VolumeRelative {
                percent_delta: percent_delta.try_into()?,
            },
            Src::PlaybackSpeed { speed } => Self::PlaybackSpeed { speed },
        })
    }
}
