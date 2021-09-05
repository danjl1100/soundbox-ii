//! Shared payload types used in backend and frontend.

// teach me
#![deny(clippy::pedantic)]
// no unsafe
#![forbid(unsafe_code)]
// no unwrap
#![deny(clippy::unwrap_used)]
// no panic
#![deny(clippy::panic)]
// docs!
#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

macro_rules! serde_derive_unidirectional {
    (
        $($from:literal => $to:literal {
            $($item:item)+
        })+
    ) => {
        $(
            $(
                #[cfg_attr(feature = $from, derive(serde::Serialize))]
                #[cfg_attr(feature = $to, derive(serde::Deserialize))]
                $item
            )+
        )+
    };
}
/// Shutdown signal
#[must_use]
#[derive(Clone, Copy)]
pub struct Shutdown;

/// Un-instantiable type
pub enum Never {}

/// Timestamp for receiving or sending a message
pub type Time = chrono::DateTime<chrono::offset::Utc>;

#[cfg(feature = "time_now")]
/// Current timestamp
#[must_use]
pub fn time_now() -> Time {
    chrono::Utc::now()
}

/// Testing "awesome number" type
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct Number {
    /// A number
    pub value: u32,
    /// String label
    pub title: String,
    /// Extra info about number, why not!
    pub is_even: bool,
}

serde_derive_unidirectional! {
    "client" => "server" {
        /// Message sent from client to server
        #[derive(Debug)]
        pub enum ClientRequest {
            /// Verification of open socket
            Heartbeat,
            /// Command
            Command(Command),
        }

        /// Command for the player
        #[derive(Debug, Clone)]
        pub enum Command {
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
            /// Set the playback speed
            PlaybackSpeed {
                /// Speed on unit scale (1.0 = normal speed)
                speed: f64,
            },
        }
    }

    "server" => "client" {
        /// Message sent from server to client
        #[derive(Debug)]
        pub enum ServerResponse {
            /// Verification of open socket
            Heartbeat,
            /// Success performing a command
            Success,
            /// Notification that the Client sourcecode changed
            ClientCodeChanged,
            /// Error message, internal to the server
            ServerError(String),
            /// Playback Status
            PlaybackStatus(PlaybackStatus),
        }
        /// Status of Playback
        #[must_use]
        #[derive(Debug, Clone)]
        pub struct PlaybackStatus {
            /// Information about the current item
            pub information: Option<PlaybackInfo>,
            /// Duration of the current song in seconds
            pub duration: u64,
            /// Fractional position within the current item (unit scale)
            pub position: f64,
            /// Playback rate (unit scale)
            pub rate: f64,
            /// State of playback
            pub state: PlaybackState,
            /// Position within the current time (seconds)
            pub time: u64,
            /// Volume percentage
            pub volume_percent: u16,
        }
        /// Information about the current (playing/paused) item
        #[must_use]
        #[derive(Debug, Clone)]
        #[allow(missing_docs)]
        pub struct PlaybackInfo {
            pub title: String,
            pub artist: String,
            pub album: String,
            pub date: String,
            pub track_number: String,
            pub track_total: String,
            /// Playlist ID of the item
            pub playlist_item_id: Option<u64>,
        }
        /// Mode of the playback
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum PlaybackState {
            /// Paused
            Paused,
            /// Playing
            Playing,
            /// Stopped
            Stopped,
        }
    }
}
impl ServerResponse {
    /// Constructs a `ServerRespone` from a result type
    ///
    /// Note: Not a [`From`] impl, due to overlapping trait bounds
    pub fn from_result<E>(result: Result<(), E>) -> Self
    where
        E: std::error::Error,
    {
        match result {
            Ok(()) => Self::Success,
            Err(e) => Self::ServerError(e.to_string()),
        }
    }
}
impl From<PlaybackStatus> for ServerResponse {
    fn from(other: PlaybackStatus) -> Self {
        Self::PlaybackStatus(other)
    }
}
impl From<Shutdown> for ServerResponse {
    fn from(_: Shutdown) -> Self {
        Self::ServerError("server is shutting down".to_string())
    }
}

// // no logic in this crate, just data types!
// #[cfg(test)]
// mod tests {
// }
