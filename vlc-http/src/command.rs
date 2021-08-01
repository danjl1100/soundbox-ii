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
    /// Seek within the current item
    SeekTo {
        /// Seconds within the current item
        seconds: u32,
    },
    /// Set the playback volume
    Volume {
        /// Percentage for the volume (clamped at 300, which means 300% volume)
        percent: u16,
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
pub enum Query {
    /// Album Artwork for the current item
    Art,
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
/// Description of a request to be executed
#[must_use]
#[allow(missing_docs)]
#[derive(Debug, PartialEq, Eq)]
pub enum RequestIntent<'a, 'b> {
    Status {
        command: &'a str,
        args: Vec<(&'b str, String)>,
    },
    Art {
        id: Option<String>,
    },
    Playlist {
        command: &'a str,
        args: Vec<(&'b str, String)>,
    },
}
impl<'a, 'b> RequestIntent<'a, 'b> {
    pub(crate) fn status(command: &'a str) -> Self {
        Self::Status {
            command,
            args: vec![],
        }
    }
    fn encode_volume_val(percent: u16) -> u32 {
        let based_256 = f32::from(percent * 256) / 100.0;
        #[allow(clippy::cast_possible_truncation)] // target size comfortably fits `u16 * 2.56`
        #[allow(clippy::cast_sign_loss)] // value is always non-negative `u16 * 2.56`
        {
            based_256.round() as u32
        }
    }
}
impl<'a, 'b> From<Command> for RequestIntent<'a, 'b> {
    /// Creates a request for the specified command
    fn from(command: Command) -> Self {
        match command {
            Command::PlaylistAdd { uri } => RequestIntent::Playlist {
                command: "in_enqueue",
                args: vec![("input", uri)],
            },
            Command::PlaylistPlay { item_id } => RequestIntent::Status {
                command: "pl_play",
                args: item_id.map(|id| vec![("id", id)]).unwrap_or_default(),
            },
            Command::PlaybackResume => RequestIntent::status("pl_forceresume"),
            Command::PlaybackPause => RequestIntent::status("pl_forcepause"),
            Command::PlaybackStop => RequestIntent::status("pl_stop"),
            Command::SeekNext => RequestIntent::status("pl_next"),
            Command::SeekPrevious => RequestIntent::status("pl_previous"),
            Command::SeekTo { seconds } => RequestIntent::Status {
                command: "seek",
                args: vec![("val", seconds.to_string())],
            },
            Command::Volume { percent } => RequestIntent::Status {
                command: "volume",
                args: vec![("val", Self::encode_volume_val(percent).to_string())],
            },
            Command::PlaybackSpeed { speed } => RequestIntent::Status {
                command: "rate",
                args: vec![("val", speed.to_string())],
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn assert_encode_simple(cmd: Command, expected: RequestIntent) {
        assert_eq!(RequestIntent::from(cmd), expected);
    }
    #[test]
    fn execs_simple() {
        assert_encode_simple(
            Command::PlaylistPlay { item_id: None },
            RequestIntent::Status {
                command: "pl_play",
                args: vec![],
            },
        );
        assert_encode_simple(
            Command::PlaybackResume,
            RequestIntent::Status {
                command: "pl_forceresume",
                args: vec![],
            },
        );
        assert_encode_simple(
            Command::PlaybackPause,
            RequestIntent::Status {
                command: "pl_forcepause",
                args: vec![],
            },
        );
        assert_encode_simple(
            Command::PlaybackStop,
            RequestIntent::Status {
                command: "pl_stop",
                args: vec![],
            },
        );
        assert_encode_simple(
            Command::SeekNext,
            RequestIntent::Status {
                command: "pl_next",
                args: vec![],
            },
        );
        assert_encode_simple(
            Command::SeekPrevious,
            RequestIntent::Status {
                command: "pl_previous",
                args: vec![],
            },
        );
        assert_encode_simple(
            Command::SeekTo { seconds: 259 },
            RequestIntent::Status {
                command: "seek",
                args: vec![("val", "259".to_string())],
            },
        );
        assert_encode_simple(
            Command::PlaybackSpeed { speed: 0.21 },
            RequestIntent::Status {
                command: "rate",
                args: vec![("val", "0.21".to_string())],
            },
        );
    }
    #[test]
    fn exec_url_encoded() {
        let uri_str = String::from("SENTINEL_ _URI_%^$");
        assert_encode_simple(
            Command::PlaylistAdd {
                uri: uri_str.clone(),
            },
            RequestIntent::Playlist {
                command: "in_enqueue",
                args: vec![("input", uri_str)],
            },
        );
        let id_str = String::from("some id");
        assert_encode_simple(
            Command::PlaylistPlay {
                item_id: Some(id_str.clone()),
            },
            RequestIntent::Status {
                command: "pl_play",
                args: vec![("id", id_str)],
            },
        );
    }
    #[test]
    fn exec_volume() {
        let percent_vals = [
            (100, "256"),
            (0, "0"),
            (20, "51"),  // round: 51.2 --> 21
            (40, "102"), // round: 102.4 --> 102
            (60, "154"), // round: 153.6 --> 154
            (80, "205"), // round: 204.8 --> 205
            (200, "512"),
        ];
        for (percent, val) in &percent_vals {
            assert_encode_simple(
                Command::Volume { percent: *percent },
                RequestIntent::Status {
                    command: "volume",
                    args: vec![("val", format!("{}", val))],
                },
            );
        }
    }
}
