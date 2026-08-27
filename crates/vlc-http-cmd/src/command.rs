// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//
//! Low-level control command types for VLC (correspond to a single API call)

/// Low-level control commands that correspond to a single API call to VLC.
///
/// See also: [`crate::goal`] provides higher-level controls
#[derive(Clone, PartialEq)]
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
    ///
    /// See also: [`crate::goal::PlaybackMode::set_random`]
    ToggleRandom,
    /// Repeats one VLC item when toggled to `true`
    ///
    /// See also: [`crate::goal::PlaybackMode::set_repeat`]
    ToggleRepeatOne,
    /// Repeats the VLC playlist when toggled to `true`
    ///
    /// See also: [`crate::goal::PlaybackMode::set_repeat`]
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
        seconds_delta: SecondsDelta,
    },
    /// Set the playback volume
    Volume {
        /// Percentage for the volume (clamped at 300, which means 300% volume)
        percent: VolumePercent,
    },
    /// Adjust the playback volume
    VolumeRelative {
        /// Percentage delta for the volume
        percent_delta: VolumePercentDelta,
    },
    /// Set the playback speed
    PlaybackSpeed {
        /// Speed on unit scale (1.0 = normal speed)
        speed: f64,
    },
}

pub use volume::Percent as VolumePercent;
pub use volume::PercentDelta as VolumePercentDelta;
mod volume {
    //! Encapsulation boundary for the numeric limits on the volume types
    //!
    //! Invariants:
    //! - [`Percent`] value is within 0 to 300 (inclusive)
    //! - [`PercentDelta`] value is within -300 to 300 (inclusive)

    use super::VolumeBoundsError;

    pub(crate) const MAX_INCLUSIVE: u16 = 300;

    /// Volume percentage clamped to 0 - 300% (inclusive)
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct Percent(pub mut(self) u16);
    impl Percent {
        /// Constructor for volume percentage
        ///
        /// # Errors
        /// Returns an error if the percent is out of bounds
        ///
        /// ```
        /// use vlc_http_cmd::command::VolumePercent;
        /// assert!(VolumePercent::new(300).is_ok());
        ///
        /// assert!(VolumePercent::new(301).is_err());
        /// ```
        pub fn new(percent: u16) -> Result<Self, VolumeBoundsError> {
            (percent <= MAX_INCLUSIVE)
                .then_some(Self(percent))
                .ok_or(VolumeBoundsError {
                    value: percent.into(),
                    signed: false,
                })
        }
    }

    /// Volume percentage delta clamped to +/- 300% (inclusive)
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct PercentDelta(pub mut(self) i16);
    impl PercentDelta {
        /// Constructor for volume percentage delta
        ///
        /// # Errors
        /// Returns an error if the percent delta is out of bounds
        ///
        /// ```
        /// use vlc_http_cmd::command::VolumePercentDelta;
        /// assert!(VolumePercentDelta::new(300).is_ok());
        /// assert!(VolumePercentDelta::new(-300).is_ok());
        ///
        /// assert!(VolumePercentDelta::new(301).is_err());
        /// assert!(VolumePercentDelta::new(-301).is_err());
        /// ```
        pub fn new(delta: i16) -> Result<Self, VolumeBoundsError> {
            (i32::from(delta.abs()) <= i32::from(MAX_INCLUSIVE))
                .then_some(Self(delta))
                .ok_or(VolumeBoundsError {
                    value: delta.into(),
                    signed: true,
                })
        }
        /// Equivalent to [`i16::unsigned_abs`]
        #[expect(clippy::missing_panics_doc, reason = "invariant of type")]
        #[must_use]
        pub fn unsigned_abs(self) -> Percent {
            let Self(value) = self;
            let magnitude = value.unsigned_abs();
            Percent::new(magnitude).expect("identical bounds for delta and percent")
        }
    }
}

/// Error in constructing a volume type
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VolumeBoundsError {
    value: i32,
    signed: bool,
}
impl std::fmt::Display for VolumeBoundsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { value, signed } = *self;
        let range_prefix = if signed { "+/-" } else { "0-" };
        write!(
            f,
            "volume value {value} out of range ({range_prefix}{})",
            volume::MAX_INCLUSIVE
        )
    }
}
impl std::error::Error for VolumeBoundsError {}

impl TryFrom<u16> for VolumePercent {
    type Error = VolumeBoundsError;
    fn try_from(value: u16) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl TryFrom<i16> for VolumePercentDelta {
    type Error = VolumeBoundsError;
    fn try_from(value: i16) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

/// Newtype for a relative number of seconds
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SecondsDelta(pub i32);
impl From<i32> for SecondsDelta {
    fn from(value: i32) -> Self {
        Self(value)
    }
}
impl std::fmt::Display for SecondsDelta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self(seconds_delta) = *self;
        write!(f, "{seconds_delta:+}")
    }
}

impl std::fmt::Debug for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // manual implementation to simplify `url::Url` to Display (Url's Debug is too verbose)
        match self {
            Self::PlaylistAdd { url } => f
                .debug_struct("PlaylistAdd")
                .field("url", &crate::url_fmt::DebugUrlRef(url))
                .finish(),
            Self::PlaylistDelete { item_id } => f
                .debug_struct("PlaylistDelete")
                .field("item_id", item_id)
                .finish(),
            Self::PlaylistPlay { item_id } => f
                .debug_struct("PlaylistPlay")
                .field("item_id", item_id)
                .finish(),
            Self::ToggleRandom => write!(f, "ToggleRandom"),
            Self::ToggleRepeatOne => write!(f, "ToggleRepeatOne"),
            Self::ToggleLoopAll => write!(f, "ToggleLoopAll"),
            Self::PlaybackResume => write!(f, "PlaybackResume"),
            Self::PlaybackPause => write!(f, "PlaybackPause"),
            Self::PlaybackStop => write!(f, "PlaybackStop"),
            Self::SeekNext => write!(f, "SeekNext"),
            Self::SeekPrevious => write!(f, "SeekPrevious"),
            Self::SeekTo { seconds } => f.debug_struct("SeekTo").field("seconds", seconds).finish(),
            Self::SeekRelative { seconds_delta } => f
                .debug_struct("SeekRelative")
                .field("seconds_delta", seconds_delta)
                .finish(),
            Self::Volume { percent } => f.debug_struct("Volume").field("percent", percent).finish(),
            Self::VolumeRelative { percent_delta } => f
                .debug_struct("VolumeRelative")
                .field("percent_delta", percent_delta)
                .finish(),
            Self::PlaybackSpeed { speed } => f
                .debug_struct("PlaybackSpeed")
                .field("speed", speed)
                .finish(),
        }
    }
}
