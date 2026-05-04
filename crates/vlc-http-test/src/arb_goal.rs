//! [`arbitrary::Arbitrary`] types for fuzz testing `vlc-http`

// TODO remove if unused
// mod alphanum_string;

use std::str::FromStr as _;
use vlc_http::{Goal, goal::TargetPlaylistItems, url::Url};

pub use self::arb_repeat_mode::ArbRepeatMode;
use self::ascii_string::AsciiString;

mod arb_repeat_mode;
mod ascii_string;

/// Arbitrary high level [`Goal`] to apply to VLC
#[derive(Clone, Debug, arbitrary::Arbitrary)]
#[expect(missing_docs, reason = "self explanatory")]
pub enum ArbGoal {
    PlaybackMode {
        repeat: ArbRepeatMode,
        is_random: bool,
    },
    PlaylistSet {
        items: Vec<AsciiString>,
    },
}

impl ArbGoal {
    /// Measure the expected number of steps for reaching the goal
    #[must_use]
    pub fn get_complexity(&self, current_len: usize) -> usize {
        match self {
            ArbGoal::PlaybackMode {
                repeat: _,
                is_random: _,
            } => 4,
            ArbGoal::PlaylistSet { items } => 2 * items.len() + current_len + 4,
        }
    }
}

impl From<ArbGoal> for Goal {
    fn from(value: ArbGoal) -> Self {
        match value {
            ArbGoal::PlaybackMode { repeat, is_random } => vlc_http::goal::PlaybackMode::new()
                .set_repeat(repeat.into())
                .set_random(is_random)
                .into(),
            ArbGoal::PlaylistSet { items } => {
                let items = items
                    .into_iter()
                    .map(|s| Url::from_str(&format!("file:///{s}")).expect("valid URL"))
                    .collect();
                TargetPlaylistItems::new().set_urls(items).into()
            }
        }
    }
}
