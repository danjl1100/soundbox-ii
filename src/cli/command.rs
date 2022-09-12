// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use crate::Shutdown;
use std::num::NonZeroUsize;
use vlc_http::{self, Action, Command, PlaybackStatus, PlaylistInfo, ResultReceiver};

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
        Ok(Ok(match self {
            Self::Play => Command::PlaybackResume.into(),
            Self::Pause => Command::PlaybackPause.into(),
            Self::Stop => Command::PlaybackStop.into(),
            Self::Add { url } => {
                let url = parse_url(&url)?;
                Command::PlaylistAdd { url }.into()
            }
            Self::Delete { item_id } => Command::PlaylistDelete { item_id }.into(),
            Self::PlaylistSet {
                urls,
                max_history_count,
            } => {
                let urls = urls.iter().map(parse_url).collect::<Result<Vec<_>, _>>()?;
                Command::PlaylistSet {
                    urls,
                    max_history_count,
                }
                .into()
            }
            Self::Start { item_id } => Command::PlaylistPlay { item_id }.into(),
            Self::Next => Command::SeekNext.into(),
            Self::Prev => Command::SeekPrevious.into(),
            Self::Seek { seconds } => Command::SeekTo { seconds }.into(),
            Self::SeekRel { seconds_delta } => Command::SeekRelative { seconds_delta }.into(),
            Self::Vol { percent } => Command::Volume { percent }.into(),
            Self::VolRel { percent_delta } => Command::VolumeRelative { percent_delta }.into(),
            Self::PlaybackMode { repeat, random } => {
                let repeat = repeat.into();
                Command::PlaybackMode { repeat, random }.into()
            }
            Self::Speed { speed } => Command::PlaybackSpeed { speed }.into(),
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

/// Unifying type for all [`Action`]s, with the respective [`ResultReceiver`]
pub enum ActionAndReceiver {
    Command(Action, ResultReceiver<()>),
    QueryPlaybackStatus(Action, ResultReceiver<PlaybackStatus>),
    QueryPlaylistInfo(Action, ResultReceiver<PlaylistInfo>),
}
impl From<Command> for ActionAndReceiver {
    fn from(command: Command) -> Self {
        use vlc_http::IntoAction;
        let (action, result_rx) = command.to_action_rx();
        Self::Command(action, result_rx)
    }
}
impl ActionAndReceiver {
    fn query_playback_status() -> Self {
        let (action, result_rx) = Action::query_playback_status();
        Self::QueryPlaybackStatus(action, result_rx)
    }
    fn query_playlist_info() -> Self {
        let (action, result_rx) = Action::query_playlist_info();
        Self::QueryPlaylistInfo(action, result_rx)
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
