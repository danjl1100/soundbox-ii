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
            /// Success performing a command
            Success,
            /// Error message, internal to the server
            ServerError(String),
            // /// Playback Status
            // PlaybackStatus(PlaybackStatus),
        }
        /// Status of Playback
        pub struct PlaybackStatus {
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
        /// Mode of the playback
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
            Err(e) => Self::from(e),
        }
    }
}
impl<E: std::error::Error> From<E> for ServerResponse {
    fn from(error: E) -> Self {
        let message = error.to_string();
        Self::ServerError(message)
    }
}

// // no logic in this crate, just data types!
// #[cfg(test)]
// mod tests {
// }
