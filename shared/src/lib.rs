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
/// Difference between timestamps
pub type TimeDifference = chrono::Duration;

#[cfg(feature = "time_now")]
/// Current timestamp
#[must_use]
pub fn time_now() -> Time {
    chrono::Utc::now()
}

/// Timestamp from specified seconds sinch epoch (useful for tests)
#[must_use]
pub fn time_from_secs(secs: i64) -> Time {
    use chrono::{offset::Utc, DateTime, NaiveDateTime};
    DateTime::from_utc(NaiveDateTime::from_timestamp(secs, 0), Utc)
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
            /// Volume percentage
            pub volume_percent: u16,
            /// Playback Timing information
            pub timing: PlaybackTiming,
        }
        /// Time-related information of playback
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct PlaybackTiming {
            /// Duration of the current song in seconds
            pub duration_secs: u64,
            /// Fractional position within the current item (unit scale)
            pub position_fraction: PositionFraction,
            /// Playback rate (unit scale)
            pub rate_ratio: RateRatio,
            /// State of playback
            pub state: PlaybackState,
            /// Position within the current time (seconds)
            pub position_secs: u64,
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
macro_rules! cheap_float_eq {
    (
        $(
            $(#[$attr:meta])*
            $vis:vis struct $name:ident (pub $float_ty:ty );
        )+
    ) => {
        $(
            $(#[$attr])*
            #[derive(PartialOrd)]
            #[serde(transparent)]
            $vis struct $name ( pub $float_ty );
            impl PartialEq for $name {
                fn eq(&self, rhs: &Self) -> bool {
                    let Self(lhs) = self;
                    let Self(rhs) = rhs;
                    let max = lhs.abs().max(rhs.abs());
                    (lhs - rhs).abs() <= (max * <$float_ty>::EPSILON)
                }
            }
            impl Eq for $name {}
            impl From<$float_ty> for $name {
                fn from(val: $float_ty) -> Self {
                    Self(val)
                }
            }
            impl From<$name> for $float_ty {
                fn from($name(val): $name) -> Self {
                    val
                }
            }
        )+
    }
}
cheap_float_eq! {
    #[derive(Debug, Default, Clone, Copy, serde::Serialize, serde::Deserialize)]
    /// Fractional position within the current item (unit scale)
    pub struct PositionFraction(pub f64);

    /// Fraction Rate (unit scale)
    #[derive(Debug, Default, Clone, Copy, serde::Serialize, serde::Deserialize)]
    pub struct RateRatio(pub f64);
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

impl PlaybackTiming {
    /// Predicts the position and time changed by the specified [`TimeDifference`]
    #[must_use]
    pub fn predict_change(self, age: TimeDifference) -> Self {
        if self.state == PlaybackState::Playing {
            // calculate age of the information
            #[allow(clippy::cast_precision_loss)]
            let age_seconds_float = (age.num_milliseconds() as f64) / 1000.0;
            #[allow(clippy::cast_possible_truncation)]
            #[allow(clippy::cast_sign_loss)]
            let age_seconds = age_seconds_float.ceil().abs() as u64;
            //
            let position_fraction = {
                #[allow(clippy::cast_precision_loss)]
                let duration = self.duration_secs as f64;
                let stored = f64::from(self.position_fraction);
                // predict
                let predict = stored + (age_seconds_float / duration);
                PositionFraction(predict.min(1.0))
            };
            let position_secs = {
                let stored = self.position_secs;
                let predict = stored + age_seconds;
                predict.min(self.duration_secs)
            };
            Self {
                position_fraction,
                position_secs,
                ..self
            }
        } else {
            self
        }
    }
}

impl Default for PlaybackTiming {
    fn default() -> Self {
        Self {
            duration_secs: Default::default(),
            rate_ratio: RateRatio::default(),
            position_fraction: PositionFraction::default(),
            state: PlaybackState::default(),
            position_secs: Default::default(),
        }
    }
}
impl Default for PlaybackState {
    fn default() -> Self {
        Self::Playing
    }
}
// // no logic in this crate, just data types!
// #[cfg(test)]
// mod tests {
// }
