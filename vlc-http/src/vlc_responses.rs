use crate::command;

use serde::Deserialize;
use std::collections::HashMap;

/// Status of the current playback
#[derive(Debug)]
#[allow(missing_docs)]
pub struct PlaybackStatus {
    /// version of the VLC-HTTP interface api
    pub apiversion: u32,
    /// Information about the current item
    pub information: Option<PlaybackInfo>,
    /// Length of the current song in seconds
    pub length: u64,
    /// True if playlist-loop is enabled
    pub is_loop: bool,
    /// Fractional position within the current item (unit scale)
    pub position: f64,
    /// True if playlist-randomize is enabled
    pub is_random: bool,
    /// Playback rate (unit scale)
    pub rate: f64,
    /// True if single-item-repeat is enabled
    pub is_repeat: bool,
    /// State of playback
    pub state: PlaybackState,
    /// Position within the current time (seconds)
    pub time: u64,
    /// VLC version string
    pub version: String,
    /// Volume percentage
    pub volume_percent: u16,
}
impl PlaybackStatus {
    pub(crate) fn from_slice(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        let json_struct: PlaybackStatusJSON = serde_json::from_slice(bytes)?;
        Ok(Self::from(json_struct))
    }
}
impl From<PlaybackStatusJSON> for PlaybackStatus {
    fn from(other: PlaybackStatusJSON) -> Self {
        let PlaybackStatusJSON {
            apiversion,
            information,
            length,
            is_loop,
            position,
            is_random,
            rate,
            is_repeat,
            state,
            time,
            version,
            volume,
        } = other;
        Self {
            apiversion,
            information: information.map(PlaybackInfo::from),
            length,
            is_loop,
            position,
            is_random,
            rate,
            is_repeat,
            state,
            time,
            version,
            volume_percent: command::decode_volume_to_percent(volume),
        }
    }
}

#[derive(Deserialize, Debug)]
pub(crate) struct PlaybackStatusJSON {
    apiversion: u32,
    information: Option<PlaybackInfoJSON>,
    length: u64,
    #[serde(rename = "loop")]
    is_loop: bool,
    position: f64,
    #[serde(rename = "random")]
    is_random: bool,
    rate: f64,
    #[serde(rename = "repeat")]
    is_repeat: bool,
    state: PlaybackState,
    time: u64,
    version: String,
    /// 256-scale
    volume: u32,
}
/// Mode of the playback
#[derive(Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum PlaybackState {
    /// Paused
    Paused,
    /// Playing
    Playing,
    /// Stopped
    Stopped,
}
#[derive(Deserialize, Debug)]
pub(crate) struct PlaybackInfoJSON {
    category: PlaybackCategoryJSON,
}
#[derive(Deserialize, Debug)]
pub(crate) struct PlaybackCategoryJSON {
    meta: PlaybackInfo,
}
/// Information about the current (playing/paused) item
#[derive(Deserialize, Debug)]
#[allow(missing_docs)]
pub struct PlaybackInfo {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub date: String,
    pub track_number: String,
    pub track_total: String,
    #[serde(flatten)]
    pub extra: HashMap<String, String>,
}
impl From<PlaybackInfoJSON> for PlaybackInfo {
    fn from(other: PlaybackInfoJSON) -> Self {
        other.category.meta
    }
}
