/// Control commands for VLC
#[derive(Debug, Clone)]
pub enum Command {
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
    // /// Set the item selection mode
    // /// TODO: deleteme in Phase 2
    // PlaybackMode {
    //     #[allow(missing_docs)]
    //     repeat: RepeatMode,
    //     /// Randomizes the VLC playback order when `true`
    //     random: bool,
    // },
    /// Set the playback speed
    PlaybackSpeed {
        /// Speed on unit scale (1.0 = normal speed)
        speed: f64,
    },
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
pub enum RepeatMode {
    /// Stop the VLC queue after playing all items
    Off,
    /// Repeat the VLC queue after playing all items
    All,
    /// Repeat only the current item
    One,
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
    let magnitude: u16 = volume_delta.abs() as u16;
    let magnitude = encode_volume_val(magnitude);
    format!("{}{}", sign_char, magnitude)
}
impl<'a, 'b> From<Command> for RequestIntent<'a, 'b> {
    /// Creates a request for the specified command
    fn from(command: Command) -> Self {
        match command {
            Command::PlaylistAdd { uri } => RequestIntent::Playlist(Some(CmdArgs {
                command: "in_enqueue",
                args: vec![("input", uri)],
            })),
            Command::PlaylistPlay { item_id } => RequestIntent::Status(Some(CmdArgs {
                command: "pl_play",
                args: item_id.map(|id| vec![("id", id)]).unwrap_or_default(),
            })),
            Command::PlaybackResume => RequestIntent::status("pl_forceresume"),
            Command::PlaybackPause => RequestIntent::status("pl_forcepause"),
            Command::PlaybackStop => RequestIntent::status("pl_stop"),
            Command::SeekNext => RequestIntent::status("pl_next"),
            Command::SeekPrevious => RequestIntent::status("pl_previous"),
            Command::SeekTo { seconds } => RequestIntent::Status(Some(CmdArgs {
                command: "seek",
                args: vec![("val", seconds.to_string())],
            })),
            Command::SeekRelative { seconds_delta } => RequestIntent::Status(Some(CmdArgs {
                command: "seek",
                args: vec![("val", fmt_seconds_delta(seconds_delta))],
            })),
            Command::Volume { percent } => RequestIntent::Status(Some(CmdArgs {
                command: "volume",
                args: vec![("val", encode_volume_val(percent).to_string())],
            })),
            Command::VolumeRelative { percent_delta } => RequestIntent::Status(Some(CmdArgs {
                command: "volume",
                args: vec![("val", fmt_volume_delta(percent_delta))],
            })),
            Command::PlaybackSpeed { speed } => RequestIntent::Status(Some(CmdArgs {
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

impl From<shared::Command> for Command {
    fn from(other: shared::Command) -> Self {
        use shared::Command;
        match other {
            Command::PlaybackResume => Self::PlaybackResume,
            Command::PlaybackPause => Self::PlaybackPause,
            Command::PlaybackStop => Self::PlaybackStop,
            Command::SeekNext => Self::SeekNext,
            Command::SeekPrevious => Self::SeekPrevious,
            Command::SeekTo { seconds } => Self::SeekTo { seconds },
            Command::SeekRelative { seconds_delta } => Self::SeekRelative { seconds_delta },
            Command::Volume { percent } => Self::Volume { percent },
            Command::VolumeRelative { percent_delta } => Self::VolumeRelative { percent_delta },
            Command::PlaybackSpeed { speed } => Self::PlaybackSpeed { speed },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn assert_encode<'a, 'b, T>(value: T, expected: RequestIntent)
    where
        RequestIntent<'a, 'b>: From<T>,
    {
        assert_eq!(RequestIntent::from(value), expected);
    }
    #[test]
    fn execs_simple_cmds() {
        assert_encode(
            Command::PlaylistPlay { item_id: None },
            RequestIntent::Status(Some(CmdArgs {
                command: "pl_play",
                args: vec![],
            })),
        );
        assert_encode(
            Command::PlaybackResume,
            RequestIntent::Status(Some(CmdArgs {
                command: "pl_forceresume",
                args: vec![],
            })),
        );
        assert_encode(
            Command::PlaybackPause,
            RequestIntent::Status(Some(CmdArgs {
                command: "pl_forcepause",
                args: vec![],
            })),
        );
        assert_encode(
            Command::PlaybackStop,
            RequestIntent::Status(Some(CmdArgs {
                command: "pl_stop",
                args: vec![],
            })),
        );
        assert_encode(
            Command::SeekNext,
            RequestIntent::Status(Some(CmdArgs {
                command: "pl_next",
                args: vec![],
            })),
        );
        assert_encode(
            Command::SeekPrevious,
            RequestIntent::Status(Some(CmdArgs {
                command: "pl_previous",
                args: vec![],
            })),
        );
        assert_encode(
            Command::SeekTo { seconds: 259 },
            RequestIntent::Status(Some(CmdArgs {
                command: "seek",
                args: vec![("val", "259".to_string())],
            })),
        );
        assert_encode(
            Command::SeekRelative { seconds_delta: 32 },
            RequestIntent::Status(Some(CmdArgs {
                command: "seek",
                args: vec![("val", "+32".to_string())],
            })),
        );
        assert_encode(
            Command::SeekRelative { seconds_delta: -57 },
            RequestIntent::Status(Some(CmdArgs {
                command: "seek",
                args: vec![("val", "-57".to_string())],
            })),
        );
        assert_encode(
            Command::SeekRelative { seconds_delta: 0 },
            RequestIntent::Status(Some(CmdArgs {
                command: "seek",
                args: vec![("val", "+0".to_string())],
            })),
        );
        assert_encode(
            Command::PlaybackSpeed { speed: 0.21 },
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
            Command::PlaylistAdd {
                uri: uri_str.clone(),
            },
            RequestIntent::Playlist(Some(CmdArgs {
                command: "in_enqueue",
                args: vec![("input", uri_str)],
            })),
        );
        let id_str = String::from("some id");
        assert_encode(
            Command::PlaylistPlay {
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
                Command::Volume { percent: *percent },
                RequestIntent::Status(Some(CmdArgs {
                    command: "volume",
                    args: vec![("val", format!("{}", val))],
                })),
            );
            let percent_signed = i16::try_from(*percent).expect("test values within range");
            let check_relative = |sign, percent_delta| {
                assert_encode(
                    Command::VolumeRelative { percent_delta },
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
