//! [`arbitrary::Arbitrary`] types for fuzz testing `vlc-http`

// TODO remove if unused
// mod alphanum_string;

use std::str::FromStr as _;
use vlc_http::{
    Goal,
    goal::{PlaybackMode, TargetPlaylistItems},
    url::Url,
};

pub use self::arb_repeat_mode::ArbRepeatMode;
use self::ascii_string::AsciiString;

mod arb_repeat_mode;
mod ascii_string;

/// Arbitrary high level [`Goal`] to apply to VLC
#[derive(Clone, Debug, arbitrary::Arbitrary)]
#[expect(missing_docs, reason = "self explanatory")]
pub enum ArbGoal {
    PlaybackMode(ArbPlaybackMode),
    PlaylistSet(ArbTargetPlaylistItems),
}

/// Arbitrary [`PlaybackMode`]
#[derive(Clone, Debug, arbitrary::Arbitrary)]
#[expect(missing_docs, reason = "self explanatory")]
pub struct ArbPlaybackMode {
    pub repeat: ArbRepeatMode,
    pub is_random: bool,
}
/// Arbitrary [`TargetPlaylistItems`]
#[derive(Clone, Debug, arbitrary::Arbitrary)]
pub struct ArbTargetPlaylistItems {
    items: Vec<AsciiString>,
}

impl ArbGoal {
    /// Measure the expected number of steps for reaching the goal
    #[must_use]
    pub fn get_complexity(&self, current_len: usize) -> usize {
        match self {
            ArbGoal::PlaybackMode(ArbPlaybackMode {
                repeat: _,
                is_random: _,
            }) => 4,
            ArbGoal::PlaylistSet(ArbTargetPlaylistItems { items }) => {
                2 * items.len() + current_len + 4
            }
        }
    }
}

impl From<ArbPlaybackMode> for ArbGoal {
    fn from(value: ArbPlaybackMode) -> Self {
        Self::PlaybackMode(value)
    }
}
impl From<ArbTargetPlaylistItems> for ArbGoal {
    fn from(value: ArbTargetPlaylistItems) -> Self {
        Self::PlaylistSet(value)
    }
}
impl From<ArbPlaybackMode> for Goal {
    fn from(value: ArbPlaybackMode) -> Self {
        ArbGoal::from(value).into()
    }
}
impl From<ArbTargetPlaylistItems> for Goal {
    fn from(value: ArbTargetPlaylistItems) -> Self {
        ArbGoal::from(value).into()
    }
}

impl From<ArbGoal> for Goal {
    fn from(value: ArbGoal) -> Self {
        match value {
            ArbGoal::PlaybackMode(inner) => PlaybackMode::from(inner).into(),
            ArbGoal::PlaylistSet(inner) => TargetPlaylistItems::from(inner).into(),
        }
    }
}
impl From<ArbPlaybackMode> for PlaybackMode {
    fn from(value: ArbPlaybackMode) -> Self {
        let ArbPlaybackMode { repeat, is_random } = value;
        vlc_http::goal::PlaybackMode::new()
            .set_repeat(repeat.into())
            .set_random(is_random)
    }
}
impl From<ArbTargetPlaylistItems> for TargetPlaylistItems {
    fn from(value: ArbTargetPlaylistItems) -> Self {
        let ArbTargetPlaylistItems { items } = value;
        let items = items
            .into_iter()
            .map(|s| Url::from_str(&format!("file:///{s}")).expect("valid URL"))
            .collect();
        TargetPlaylistItems::new().set_urls(items)
    }
}
