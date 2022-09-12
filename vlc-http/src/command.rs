// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use std::{convert::TryFrom, num::NonZeroUsize};

pub(crate) use crate::http_client::intent::{CmdArgs, TextIntent};
use crate::http_client::intent::{PlaylistIntent, StatusIntent};

/// High-level Commands for VLC
// Internally, routed to either LowCommand (single command) or HighCommand (sequence of commands)
#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Clone)]
pub enum PublicCommand {
    /// Add the specified item to the playlist
    //TODO: move to LowCommand (only)
    PlaylistAdd {
        /// Path to the file to enqueue
        url: url::Url,
    },
    /// Deletes the specified item from the playlist
    //TODO: move to LowCommand (only)
    PlaylistDelete {
        /// Identifier of the playlist item to remove
        item_id: String,
    },
    /// Play the specified item in the playlist
    //TODO: move to LowCommand (only)
    PlaylistPlay {
        /// Identifier of the playlist item
        item_id: Option<String>,
    },
    /// Set the current playing and up-next playlist URLs, clearing the history to the specified max count.
    ///
    /// NOTE: `current_or_past_url` is accepted as previously-played if it is the most recent history item.
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
#[derive(Debug)]
pub(crate) enum HighCommand {
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
    PlaybackMode {
        #[allow(missing_docs)]
        repeat: RepeatMode,
        /// Randomizes the VLC playback order when `true`
        random: bool,
    },
}
/// Low-level Control commands for VLC
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum LowCommand {
    /// Add the specified item to the playlist
    PlaylistAdd {
        /// Path to the file to enqueue
        url: url::Url,
    },
    /// Deletes the specified item from the playlist
    PlaylistDelete {
        /// Identifier of the playlist item to remove
        item_id: String,
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
fn decode_url(url: &url::Url) -> String {
    urlencoding::decode(url.as_ref())
        .expect("UTF8 input on all URLs")
        .to_string()
}
impl From<LowCommand> for TextIntent {
    /// Creates a request for the specified command
    fn from(command: LowCommand) -> Self {
        match command {
            LowCommand::PlaylistAdd { url } => PlaylistIntent(Some(CmdArgs {
                command: "in_enqueue",
                args: vec![("input", decode_url(&url))],
            }))
            .into(),
            LowCommand::PlaylistDelete { item_id } => PlaylistIntent(Some(CmdArgs {
                command: "pl_delete",
                args: vec![("id", item_id)],
            }))
            .into(),
            LowCommand::PlaylistPlay { item_id } => StatusIntent(Some(CmdArgs {
                command: "pl_play",
                args: item_id.map(|id| vec![("id", id)]).unwrap_or_default(),
            }))
            .into(),
            LowCommand::PlaybackResume => TextIntent::status("pl_forceresume"),
            LowCommand::PlaybackPause => TextIntent::status("pl_forcepause"),
            LowCommand::PlaybackStop => TextIntent::status("pl_stop"),
            LowCommand::SeekNext => TextIntent::status("pl_next"),
            LowCommand::SeekPrevious => TextIntent::status("pl_previous"),
            LowCommand::SeekTo { seconds } => StatusIntent(Some(CmdArgs {
                command: "seek",
                args: vec![("val", seconds.to_string())],
            }))
            .into(),
            LowCommand::SeekRelative { seconds_delta } => StatusIntent(Some(CmdArgs {
                command: "seek",
                args: vec![("val", fmt_seconds_delta(seconds_delta))],
            }))
            .into(),
            LowCommand::Volume { percent } => StatusIntent(Some(CmdArgs {
                command: "volume",
                args: vec![("val", encode_volume_val(percent).to_string())],
            }))
            .into(),
            LowCommand::VolumeRelative { percent_delta } => StatusIntent(Some(CmdArgs {
                command: "volume",
                args: vec![("val", fmt_volume_delta(percent_delta))],
            }))
            .into(),
            LowCommand::ToggleRandom => TextIntent::status("pl_random"),
            LowCommand::ToggleRepeatOne => TextIntent::status("pl_repeat"),
            LowCommand::ToggleLoopAll => TextIntent::status("pl_loop"),
            LowCommand::PlaybackSpeed { speed } => StatusIntent(Some(CmdArgs {
                command: "rate",
                args: vec![("val", speed.to_string())],
            }))
            .into(),
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
            Public::PlaylistAdd { url } => Self::PlaylistAdd { url },
            Public::PlaylistDelete { item_id } => Self::PlaylistDelete { item_id },
            Public::PlaylistPlay { item_id } => Self::PlaylistPlay { item_id },
            Public::PlaylistSet {
                urls,
                max_history_count,
            } => {
                return Err(HighCommand::PlaylistSet {
                    urls,
                    max_history_count,
                });
            }
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
    #[allow(clippy::trait_duplication_in_bounds)] //TODO remove this, needed (?) for clippy 0.1.62,
                                                  // but not duplicated on playground's clippy 0.1.65
    fn assert_encode<T, U>(value: T, expected: U)
    where
        TextIntent: From<T> + From<U>,
    {
        assert_eq!(TextIntent::from(value), TextIntent::from(expected));
    }
    #[test]
    fn execs_simple_cmds() {
        assert_encode(
            LowCommand::PlaylistPlay { item_id: None },
            StatusIntent(Some(CmdArgs {
                command: "pl_play",
                args: vec![],
            })),
        );
        assert_encode(
            LowCommand::PlaybackResume,
            StatusIntent(Some(CmdArgs {
                command: "pl_forceresume",
                args: vec![],
            })),
        );
        assert_encode(
            LowCommand::PlaybackPause,
            StatusIntent(Some(CmdArgs {
                command: "pl_forcepause",
                args: vec![],
            })),
        );
        assert_encode(
            LowCommand::PlaybackStop,
            StatusIntent(Some(CmdArgs {
                command: "pl_stop",
                args: vec![],
            })),
        );
        assert_encode(
            LowCommand::SeekNext,
            StatusIntent(Some(CmdArgs {
                command: "pl_next",
                args: vec![],
            })),
        );
        assert_encode(
            LowCommand::SeekPrevious,
            StatusIntent(Some(CmdArgs {
                command: "pl_previous",
                args: vec![],
            })),
        );
        assert_encode(
            LowCommand::SeekTo { seconds: 259 },
            StatusIntent(Some(CmdArgs {
                command: "seek",
                args: vec![("val", "259".to_string())],
            })),
        );
        assert_encode(
            LowCommand::SeekRelative { seconds_delta: 32 },
            StatusIntent(Some(CmdArgs {
                command: "seek",
                args: vec![("val", "+32".to_string())],
            })),
        );
        assert_encode(
            LowCommand::SeekRelative { seconds_delta: -57 },
            StatusIntent(Some(CmdArgs {
                command: "seek",
                args: vec![("val", "-57".to_string())],
            })),
        );
        assert_encode(
            LowCommand::SeekRelative { seconds_delta: 0 },
            StatusIntent(Some(CmdArgs {
                command: "seek",
                args: vec![("val", "+0".to_string())],
            })),
        );
        assert_encode(
            LowCommand::PlaybackSpeed { speed: 0.21 },
            StatusIntent(Some(CmdArgs {
                command: "rate",
                args: vec![("val", "0.21".to_string())],
            })),
        );
    }
    #[test]
    fn exec_url_encoded() {
        let url = url::Url::parse("file:///SENTINEL_%20_URL_%20%5E%24").expect("valid url");
        assert_encode(
            LowCommand::PlaylistAdd { url },
            PlaylistIntent(Some(CmdArgs {
                command: "in_enqueue",
                args: vec![("input", "file:///SENTINEL_ _URL_ ^$".to_string())],
            })),
        );
        let id_str = String::from("some id");
        assert_encode(
            LowCommand::PlaylistDelete {
                item_id: id_str.clone(),
            },
            PlaylistIntent(Some(CmdArgs {
                command: "pl_delete",
                args: vec![("id", id_str.clone())],
            })),
        );
        assert_encode(
            LowCommand::PlaylistPlay {
                item_id: Some(id_str.clone()),
            },
            StatusIntent(Some(CmdArgs {
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
                StatusIntent(Some(CmdArgs {
                    command: "volume",
                    args: vec![("val", format!("{}", val))],
                })),
            );
            let percent_signed = i16::try_from(*percent).expect("test values within range");
            let check_relative = |sign, percent_delta| {
                assert_encode(
                    LowCommand::VolumeRelative { percent_delta },
                    StatusIntent(Some(CmdArgs {
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
}
