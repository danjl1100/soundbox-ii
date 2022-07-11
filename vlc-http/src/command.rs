// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use std::convert::TryFrom;

/// High-level Commands for VLC
#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Clone)]
pub enum PublicCommand {
    /// Add the specified item to the playlist
    //TODO: move to LowCommand (only)
    PlaylistAdd {
        /// Path to the file to enqueue
        uri: String,
    },
    /// Play the specified item in the playlist
    //TODO: move to LowCommand (only)
    PlaylistPlay {
        /// Identifier of the playlist item
        item_id: Option<String>,
    },
    /// Force playback to resume
    PlaybackResume,
    /// Force playback to pause
    PlaybackPause,
    /// Force playback to pause
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
    /// Set the item selection mode
    // TODO: deleteme in Phase 2
    PlaybackMode {
        #[allow(missing_docs)]
        repeat: RepeatMode,
        /// Randomizes the VLC playback order when `true`
        random: bool,
    },
    /// Set the playback speed
    PlaybackSpeed {
        /// Speed on unit scale (1.0 = normal speed)
        speed: f64,
    },
}
pub(crate) enum HighCommand {
    PlaybackMode {
        #[allow(missing_docs)]
        repeat: RepeatMode,
        /// Randomizes the VLC playback order when `true`
        random: bool,
    },
}
/// Low-level Control commands for VLC
#[derive(Debug, Clone)]
pub(crate) enum LowCommand {
    /// Add the specified item to the playlist
    PlaylistAdd {
        /// Path to the file to enqueue
        uri: String,
    },
    /// Play the specified item in the playlist
    PlaylistPlay {
        /// Identifier of the playlist item
        item_id: Option<String>,
    },
    /// Randomizes VLC playback order when toggled to `true`
    ToggleRandom,
    /// Repeats one VLC item when toggled to `true`
    ToggleRepeatOne,
    /// Repeats the VLC playlist when toggled to `true`
    ToggleLoopAll,
    // ========================================
    PlaybackResume,
    PlaybackPause,
    PlaybackStop,
    SeekNext,
    SeekPrevious,
    SeekTo {
        seconds: u32,
    },
    SeekRelative {
        seconds_delta: i32,
    },
    Volume {
        percent: u16,
    },
    VolumeRelative {
        percent_delta: i16,
    },
    PlaybackSpeed {
        speed: f64,
    },
}
/// Common commands (equivalent on public and low-level interfaces)
#[derive(Debug, Clone)]
pub(crate) enum SimpleCommand {
    PlaybackResume,
    PlaybackPause,
    PlaybackStop,
    SeekNext,
    SeekPrevious,
    SeekTo { seconds: u32 },
    SeekRelative { seconds_delta: i32 },
    Volume { percent: u16 },
    VolumeRelative { percent_delta: i16 },
    PlaybackSpeed { speed: f64 },
}
/// Information queries for VLC
#[derive(Debug, Clone)]
pub(crate) enum Query {
    /// Playback status for the current playing item
    PlaybackStatus,
    /// Playlist items
    PlaylistInfo,
}

/// Rule for selecting the next playback item in the VLC queue
///
/// TODO: deleteme in Phase 2
#[derive(Debug, Clone, Copy)]
pub enum RepeatMode {
    /// Stop the VLC queue after playing all items
    Off,
    /// Repeat the VLC queue after playing all items
    All,
    /// Repeat only the current item
    One,
}
impl RepeatMode {
    pub(crate) fn is_loop_all(self) -> bool {
        matches!(self, Self::All)
    }
    pub(crate) fn is_repeat_one(self) -> bool {
        matches!(self, Self::One)
    }
}

/// Type of a response
#[allow(missing_docs)]
pub enum TextResponseType {
    Status,
    Playlist,
}

/// Description of a request to be executed
#[must_use]
#[allow(missing_docs)]
#[derive(Debug, PartialEq, Eq)]
pub enum RequestIntent<'a, 'b> {
    Status(Option<CmdArgs<'a, 'b>>),
    Playlist(Option<CmdArgs<'a, 'b>>),
}
#[derive(Debug, PartialEq, Eq)]
pub struct CmdArgs<'a, 'b> {
    pub command: &'a str,
    pub args: Vec<(&'b str, String)>,
}
impl<'a, 'b> RequestIntent<'a, 'b> {
    pub(crate) fn status(command: &'a str) -> Self {
        Self::Status(Some(CmdArgs {
            command,
            args: vec![],
        }))
    }
    pub fn get_type(&self) -> TextResponseType {
        match self {
            Self::Status(_) => TextResponseType::Status,
            Self::Playlist(_) => TextResponseType::Playlist,
        }
    }
}
/// Query for Album Artwork for the current item
pub(crate) struct ArtRequestIntent {
    pub id: Option<String>,
}

pub(crate) fn encode_volume_val(percent: u16) -> u32 {
    let based_256 = f32::from(percent * 256) / 100.0;
    #[allow(clippy::cast_possible_truncation)] // target size comfortably fits `u16 * 2.56`
    #[allow(clippy::cast_sign_loss)] // value is always non-negative `u16 * 2.56`
    {
        based_256.round() as u32
    }
}
pub(crate) fn decode_volume_to_percent(based_256: u32) -> u16 {
    let based_100 = f64::from(based_256 * 100) / 256.0;
    #[allow(clippy::cast_possible_truncation)] // target size comfortably fits `u32 / 2.56`
    #[allow(clippy::cast_sign_loss)] // value is always non-negative `u32 / 2.56`
    {
        based_100.round() as u16
    }
}
fn fmt_seconds_delta(seconds_delta: i32) -> String {
    format!("{:+}", seconds_delta)
}
fn fmt_volume_delta(volume_delta: i16) -> String {
    let sign_char = if volume_delta < 0 { '-' } else { '+' };
    let magnitude: u16 = volume_delta.unsigned_abs();
    let magnitude = encode_volume_val(magnitude);
    format!("{}{}", sign_char, magnitude)
}
impl<'a, 'b> From<LowCommand> for RequestIntent<'a, 'b> {
    /// Creates a request for the specified command
    fn from(command: LowCommand) -> Self {
        match command {
            LowCommand::PlaylistAdd { uri } => RequestIntent::Playlist(Some(CmdArgs {
                command: "in_enqueue",
                args: vec![("input", uri)],
            })),
            LowCommand::PlaylistPlay { item_id } => RequestIntent::Status(Some(CmdArgs {
                command: "pl_play",
                args: item_id.map(|id| vec![("id", id)]).unwrap_or_default(),
            })),
            LowCommand::PlaybackResume => RequestIntent::status("pl_forceresume"),
            LowCommand::PlaybackPause => RequestIntent::status("pl_forcepause"),
            LowCommand::PlaybackStop => RequestIntent::status("pl_stop"),
            LowCommand::SeekNext => RequestIntent::status("pl_next"),
            LowCommand::SeekPrevious => RequestIntent::status("pl_previous"),
            LowCommand::SeekTo { seconds } => RequestIntent::Status(Some(CmdArgs {
                command: "seek",
                args: vec![("val", seconds.to_string())],
            })),
            LowCommand::SeekRelative { seconds_delta } => RequestIntent::Status(Some(CmdArgs {
                command: "seek",
                args: vec![("val", fmt_seconds_delta(seconds_delta))],
            })),
            LowCommand::Volume { percent } => RequestIntent::Status(Some(CmdArgs {
                command: "volume",
                args: vec![("val", encode_volume_val(percent).to_string())],
            })),
            LowCommand::VolumeRelative { percent_delta } => RequestIntent::Status(Some(CmdArgs {
                command: "volume",
                args: vec![("val", fmt_volume_delta(percent_delta))],
            })),
            LowCommand::ToggleRandom => RequestIntent::status("pl_random"),
            LowCommand::ToggleRepeatOne => RequestIntent::status("pl_repeat"),
            LowCommand::ToggleLoopAll => RequestIntent::status("pl_loop"),
            LowCommand::PlaybackSpeed { speed } => RequestIntent::Status(Some(CmdArgs {
                command: "rate",
                args: vec![("val", speed.to_string())],
            })),
        }
    }
}
impl<'a, 'b> From<Query> for RequestIntent<'a, 'b> {
    fn from(query: Query) -> Self {
        match query {
            Query::PlaybackStatus => RequestIntent::Status(None),
            Query::PlaylistInfo => RequestIntent::Playlist(None),
        }
    }
}

impl From<shared::Command> for PublicCommand {
    fn from(other: shared::Command) -> Self {
        use shared::Command as Shared;
        match other {
            Shared::PlaybackResume => Self::PlaybackResume,
            Shared::PlaybackPause => Self::PlaybackPause,
            Shared::PlaybackStop => Self::PlaybackStop,
            Shared::SeekNext => Self::SeekNext,
            Shared::SeekPrevious => Self::SeekPrevious,
            Shared::SeekTo { seconds } => Self::SeekTo { seconds },
            Shared::SeekRelative { seconds_delta } => Self::SeekRelative { seconds_delta },
            Shared::Volume { percent } => Self::Volume { percent },
            Shared::VolumeRelative { percent_delta } => Self::VolumeRelative { percent_delta },
            Shared::PlaybackSpeed { speed } => Self::PlaybackSpeed { speed },
        }
    }
}

impl TryFrom<PublicCommand> for LowCommand {
    type Error = HighCommand;

    fn try_from(command: PublicCommand) -> Result<Self, Self::Error> {
        use PublicCommand as Public;
        Ok(match command {
            Public::PlaylistAdd { uri } => Self::PlaylistAdd { uri },
            Public::PlaylistPlay { item_id } => Self::PlaylistPlay { item_id },
            Public::PlaybackResume => Self::PlaybackResume,
            Public::PlaybackPause => Self::PlaybackPause,
            Public::PlaybackStop => Self::PlaybackStop,
            Public::SeekNext => Self::SeekNext,
            Public::SeekPrevious => Self::SeekPrevious,
            Public::SeekTo { seconds } => Self::SeekTo { seconds },
            Public::SeekRelative { seconds_delta } => Self::SeekRelative { seconds_delta },
            Public::Volume { percent } => Self::Volume { percent },
            Public::VolumeRelative { percent_delta } => Self::VolumeRelative { percent_delta },
            Public::PlaybackSpeed { speed } => Self::PlaybackSpeed { speed },
            Public::PlaybackMode { repeat, random } => {
                return Err(HighCommand::PlaybackMode { repeat, random });
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::needless_pass_by_value)]
    fn assert_encode<'a, 'b, T>(value: T, expected: RequestIntent)
    where
        RequestIntent<'a, 'b>: From<T>,
    {
        assert_eq!(RequestIntent::from(value), expected);
    }
    #[test]
    fn execs_simple_cmds() {
        assert_encode(
            LowCommand::PlaylistPlay { item_id: None },
            RequestIntent::Status(Some(CmdArgs {
                command: "pl_play",
                args: vec![],
            })),
        );
        assert_encode(
            LowCommand::PlaybackResume,
            RequestIntent::Status(Some(CmdArgs {
                command: "pl_forceresume",
                args: vec![],
            })),
        );
        assert_encode(
            LowCommand::PlaybackPause,
            RequestIntent::Status(Some(CmdArgs {
                command: "pl_forcepause",
                args: vec![],
            })),
        );
        assert_encode(
            LowCommand::PlaybackStop,
            RequestIntent::Status(Some(CmdArgs {
                command: "pl_stop",
                args: vec![],
            })),
        );
        assert_encode(
            LowCommand::SeekNext,
            RequestIntent::Status(Some(CmdArgs {
                command: "pl_next",
                args: vec![],
            })),
        );
        assert_encode(
            LowCommand::SeekPrevious,
            RequestIntent::Status(Some(CmdArgs {
                command: "pl_previous",
                args: vec![],
            })),
        );
        assert_encode(
            LowCommand::SeekTo { seconds: 259 },
            RequestIntent::Status(Some(CmdArgs {
                command: "seek",
                args: vec![("val", "259".to_string())],
            })),
        );
        assert_encode(
            LowCommand::SeekRelative { seconds_delta: 32 },
            RequestIntent::Status(Some(CmdArgs {
                command: "seek",
                args: vec![("val", "+32".to_string())],
            })),
        );
        assert_encode(
            LowCommand::SeekRelative { seconds_delta: -57 },
            RequestIntent::Status(Some(CmdArgs {
                command: "seek",
                args: vec![("val", "-57".to_string())],
            })),
        );
        assert_encode(
            LowCommand::SeekRelative { seconds_delta: 0 },
            RequestIntent::Status(Some(CmdArgs {
                command: "seek",
                args: vec![("val", "+0".to_string())],
            })),
        );
        assert_encode(
            LowCommand::PlaybackSpeed { speed: 0.21 },
            RequestIntent::Status(Some(CmdArgs {
                command: "rate",
                args: vec![("val", "0.21".to_string())],
            })),
        );
    }
    #[test]
    fn exec_url_encoded() {
        let uri_str = String::from("SENTINEL_ _URI_%^$");
        assert_encode(
            LowCommand::PlaylistAdd {
                uri: uri_str.clone(),
            },
            RequestIntent::Playlist(Some(CmdArgs {
                command: "in_enqueue",
                args: vec![("input", uri_str)],
            })),
        );
        let id_str = String::from("some id");
        assert_encode(
            LowCommand::PlaylistPlay {
                item_id: Some(id_str.clone()),
            },
            RequestIntent::Status(Some(CmdArgs {
                command: "pl_play",
                args: vec![("id", id_str)],
            })),
        );
    }
    #[test]
    fn exec_volume() {
        use std::convert::TryFrom;
        let percent_vals = [
            (100, 256),
            (0, 0),
            (20, 51),  // round: 51.2 --> 51
            (40, 102), // round: 102.4 --> 102
            (60, 154), // round: 153.6 --> 154
            (80, 205), // round: 204.8 --> 205
            (200, 512),
        ];
        for (percent, val) in &percent_vals {
            assert_eq!(encode_volume_val(*percent), *val);
            assert_eq!(decode_volume_to_percent(*val), *percent);
            assert_encode(
                LowCommand::Volume { percent: *percent },
                RequestIntent::Status(Some(CmdArgs {
                    command: "volume",
                    args: vec![("val", format!("{}", val))],
                })),
            );
            let percent_signed = i16::try_from(*percent).expect("test values within range");
            let check_relative = |sign, percent_delta| {
                assert_encode(
                    LowCommand::VolumeRelative { percent_delta },
                    RequestIntent::Status(Some(CmdArgs {
                        command: "volume",
                        args: vec![("val", format!("{}{}", sign, val))],
                    })),
                );
            };
            if percent_signed == 0 {
                check_relative("+", percent_signed);
            } else {
                check_relative("+", percent_signed);
                check_relative("-", -percent_signed);
            }
        }
    }
    #[test]
    fn execs_simple_queries() {
        assert_encode(Query::PlaybackStatus, RequestIntent::Status(None));
        assert_encode(Query::PlaylistInfo, RequestIntent::Playlist(None));
    }
}
