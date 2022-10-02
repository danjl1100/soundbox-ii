// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use crate::{
    seq::{self, SequencerCommand},
    Shutdown,
};
use std::num::NonZeroUsize;
use vlc_http::{self, PlaybackStatus, PlaylistInfo, ResultReceiver};

pub const COMMAND_NAME: &str = "soundbox-ii";
pub const PROMPT_STR: &str = "soundbox-ii>";

/// Options for soundbox-ii interactive shell
#[derive(clap::Parser, Debug)]
#[clap(
    name = PROMPT_STR,
    no_binary_name = true,
)]
pub struct InteractiveArgs {
    #[clap(subcommand)]
    pub command: Option<Subcommand>,
}
#[derive(clap::Subcommand, Debug)]
pub enum Subcommand {
    /// Play command
    Play,
    /// Pause command
    Pause,
    /// Stop command
    Stop,
    /// Add an item to the playlist
    Add {
        /// Item to add
        url: String,
    },
    /// Deletes an item from the playlist
    Delete {
        /// Item to delete
        item_id: String,
    },
    //TODO remove from InteractiveArgs and have a rule polling a legit Sequencer, tracking track completions to trigger
    PlaylistSet {
        /// Maximum number of history (past-played) items to retain
        max_history_count: NonZeroUsize,
        /// Path to the file(s) to queue next, starting with the current/past item
        urls: Vec<String>,
    },
    /// Sequencer node subcommands
    Node {
        #[clap(subcommand)]
        command: seq::Cmd,
    },
    /// Start command
    Start {
        /// Optional item id
        item_id: Option<String>,
    },
    /// Next track command
    Next,
    /// Previous track command
    #[clap(alias("previous"))]
    Prev,
    /// Seek command (absolute)
    Seek {
        /// Absolute seconds within the current item
        seconds: u32,
    },
    /// Seek-relative command
    SeekRel {
        /// Relative seconds within the current item
        seconds_delta: i32,
    },
    /// Volume command (absolute)
    #[clap(alias("volume"))]
    Vol {
        /// Absolute volume percentage
        percent: u16,
    },
    /// Volume-relative command
    VolRel {
        /// Relative volume percentage
        percent_delta: i16,
    },
    /// Playback mode command
    #[clap(alias("mode"))]
    PlaybackMode {
        #[clap(subcommand)]
        repeat: Repeat,
        #[clap(long, alias("shuffle"))]
        /// Randomize VLC playback order
        random: bool,
    },
    /// Speed command
    Speed {
        /// Fractional speed
        speed: f64,
    },
    /// Status query
    #[clap(alias("."))]
    Status,
    /// Playlist query
    #[clap(alias("p"))]
    Playlist,
    /// Exits the interactive shell, server, and entire process
    #[clap(alias("q"), alias("exit"))]
    Quit,
    /// Show copying/license information
    Show {
        #[clap(subcommand)]
        ty: ShowCopyingLicenseType,
    },
    /// Print help information
    Help,
}
#[derive(clap::Subcommand, Debug)]
pub enum ShowCopyingLicenseType {
    /// Show warranty details
    #[clap(alias("w"))]
    Warranty,
    /// Show conditions for redistribution
    #[clap(alias("c"))]
    Copying,
}
#[derive(clap::Subcommand, Debug)]
pub enum Repeat {
    /// Repeat none
    Off,
    /// Repeat all playlist items
    All,
    /// Repeat current item only
    One,
}
impl From<Repeat> for vlc_http::RepeatMode {
    fn from(other: Repeat) -> Self {
        match other {
            Repeat::Off => Self::Off,
            Repeat::All => Self::All,
            Repeat::One => Self::One,
        }
    }
}
impl Subcommand {
    pub(super) fn try_build(self) -> Result<Result<ActionAndReceiver, Option<Shutdown>>, String> {
        use vlc_http::Command as Vlc;
        Ok(Ok(match self {
            Self::Play => Vlc::PlaybackResume.into(),
            Self::Pause => Vlc::PlaybackPause.into(),
            Self::Stop => Vlc::PlaybackStop.into(),
            Self::Add { url } => {
                let url = parse_url(&url)?;
                Vlc::PlaylistAdd { url }.into()
            }
            Self::Delete { item_id } => Vlc::PlaylistDelete { item_id }.into(),
            Self::PlaylistSet {
                urls,
                max_history_count,
            } => {
                let urls = urls.iter().map(parse_url).collect::<Result<Vec<_>, _>>()?;
                Vlc::PlaylistSet {
                    urls,
                    max_history_count,
                }
                .into()
            }
            Self::Node { command } => command.into(),
            Self::Start { item_id } => Vlc::PlaylistPlay { item_id }.into(),
            Self::Next => Vlc::SeekNext.into(),
            Self::Prev => Vlc::SeekPrevious.into(),
            Self::Seek { seconds } => Vlc::SeekTo { seconds }.into(),
            Self::SeekRel { seconds_delta } => Vlc::SeekRelative { seconds_delta }.into(),
            Self::Vol { percent } => Vlc::Volume { percent }.into(),
            Self::VolRel { percent_delta } => Vlc::VolumeRelative { percent_delta }.into(),
            Self::PlaybackMode { repeat, random } => {
                let repeat = repeat.into();
                Vlc::PlaybackMode { repeat, random }.into()
            }
            Self::Speed { speed } => Vlc::PlaybackSpeed { speed }.into(),
            Self::Status => ActionAndReceiver::query_playback_status(),
            Self::Playlist => ActionAndReceiver::query_playlist_info(),
            Self::Quit => {
                return Ok(Err(Some(Shutdown)));
            }
            Self::Show { ty } => {
                eprintln!();
                match ty {
                    ShowCopyingLicenseType::Warranty => {
                        eprintln!("{}", shared::license::WARRANTY);
                    }
                    ShowCopyingLicenseType::Copying => {
                        eprintln!("{}", shared::license::REDISTRIBUTION);
                    }
                }
                return Ok(Err(None));
            }
            Self::Help => unreachable!("built-in help displayed by clap"),
        }))
    }
}

/// Unifying type for all actions, with optional/respective [`ResultReceiver`]
pub enum ActionAndReceiver {
    VlcCommand(vlc_http::Action, ResultReceiver<()>),
    SequencerCommand(SequencerCommand),
    VlcQueryPlayback(vlc_http::Action, ResultReceiver<PlaybackStatus>),
    VlcQueryPlaylist(vlc_http::Action, ResultReceiver<PlaylistInfo>),
}
impl From<vlc_http::Command> for ActionAndReceiver {
    fn from(command: vlc_http::Command) -> Self {
        use vlc_http::IntoAction;
        let (action, result_rx) = command.to_action_rx();
        Self::VlcCommand(action, result_rx)
    }
}
impl ActionAndReceiver {
    fn query_playback_status() -> Self {
        let (action, result_rx) = vlc_http::Action::query_playback_status();
        Self::VlcQueryPlayback(action, result_rx)
    }
    fn query_playlist_info() -> Self {
        let (action, result_rx) = vlc_http::Action::query_playlist_info();
        Self::VlcQueryPlaylist(action, result_rx)
    }
}
impl From<SequencerCommand> for ActionAndReceiver {
    fn from(cmd: SequencerCommand) -> Self {
        Self::SequencerCommand(cmd)
    }
}
impl From<seq::Cmd> for ActionAndReceiver {
    fn from(cmd: seq::Cmd) -> Self {
        Self::SequencerCommand(cmd.into())
    }
}

pub fn parse_url<T>(s: T) -> Result<url::Url, String>
where
    T: AsRef<str>,
{
    fn parse_simple(s: &str) -> Result<url::Url, url::ParseError> {
        url::Url::parse(s)
    }
    fn parse_relative_cwd(s: &str) -> Option<url::Url> {
        std::env::current_dir()
            .ok()
            .and_then(|cwd| {
                cwd.to_str()
                    .and_then(|cwd| parse_simple(&format!("file://{cwd}/")).ok())
            })
            .and_then(|cwd_url| cwd_url.join(s).ok())
    }
    fn parse(s: &str) -> Result<url::Url, url::ParseError> {
        parse_simple(s).or_else(|err| parse_relative_cwd(s).ok_or(err))
    }
    let s = s.as_ref();
    parse(s).map_err(|e| format!("{e} in: {s:?}"))
}

#[cfg(test)]
mod tests {
    use super::{parse_url, InteractiveArgs};

    #[test]
    fn verify_prompt() {
        use clap::CommandFactory;
        InteractiveArgs::command().debug_assert();
    }

    #[test]
    fn parse_url_coercion() -> Result<(), String> {
        // valid absolute URL -> passthru
        assert_eq!(parse_url("file:///a.mp3")?.to_string(), "file:///a.mp3");
        assert_eq!(
            parse_url("http://a.host:3030/then/the/path/to/file.txt")?.to_string(),
            "http://a.host:3030/then/the/path/to/file.txt"
        );
        // relative URL -> adds "file" scheme and current directory
        let cwd = std::env::current_dir()
            .ok()
            .and_then(|p| p.to_str().map(ToString::to_string))
            .unwrap_or_default();
        assert_eq!(
            parse_url("simple_string_path.zip")?.to_string(),
            format!("file://{cwd}/simple_string_path.zip")
        );
        Ok(())
    }
}
