use crate::Shutdown;
use vlc_http::{self, Action, Command, PlaybackStatus, PlaylistInfo, ResultReceiver};

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
        uri: String,
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
    /// Print help information
    Help,
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
    pub(super) fn build(self) -> Result<ActionAndReceiver, Shutdown> {
        Ok(match self {
            Self::Play => Command::PlaybackResume.into(),
            Self::Pause => Command::PlaybackPause.into(),
            Self::Stop => Command::PlaybackStop.into(),
            Self::Add { uri } => Command::PlaylistAdd { uri }.into(),
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
                return Err(Shutdown);
            }
            Self::Help => unreachable!("built-in help displayed by clap"),
        })
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

#[cfg(test)]
mod tests {
    use super::InteractiveArgs;

    #[test]
    fn verify_prompt() {
        use clap::CommandFactory;
        InteractiveArgs::command().debug_assert();
    }
}
