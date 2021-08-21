//! Shared payload types used in backend and frontend.

// TODO: only while building
#![allow(dead_code)]
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

use serde::{Deserialize, Serialize};

/// Testing "awesome number" type
#[allow(missing_docs)] //TODO
#[derive(Debug, Deserialize, Serialize)]
pub struct Number {
    pub value: u32,
    pub title: String,
    pub is_even: bool,
}

/// Message sent from client to server
#[derive(Debug)]
#[cfg_attr(feature = "client", derive(Serialize))]
#[cfg_attr(feature = "server", derive(Deserialize))]
#[allow(missing_docs)] //TODO
pub enum ClientRequest {
    Command(Command),
}
/// Message sent from server to client
#[derive(Debug)]
#[cfg_attr(feature = "client", derive(Deserialize))]
#[cfg_attr(feature = "server", derive(Serialize))]
#[allow(missing_docs)] //TODO
pub enum ServerResponse {
    Success,
}

/// Command for the player
#[derive(Debug, Clone)]
#[cfg_attr(feature = "client", derive(Serialize))]
#[cfg_attr(feature = "server", derive(Deserialize))]
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

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
