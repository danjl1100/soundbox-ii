// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use crate::command::VolumePercent256;
use serde::Deserialize;
use std::collections::BTreeMap;

/// Status of the current playback
#[must_use]
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(test, derive(serde::Serialize))]
pub struct Status {
    /// version of the VLC-HTTP interface api
    pub apiversion: u32,
    /// Information about the current item
    pub information: Option<Info>,
    /// True if playlist-loop is enabled
    pub is_loop_all: bool,
    /// True if playlist-randomize is enabled
    pub is_random: bool,
    /// True if single-item-repeat is enabled
    pub is_repeat_one: bool,
    /// VLC version string
    pub version: String,
    /// Volume percentage
    pub volume_percent: u16,
    /// Mode of playback
    pub mode: Mode,
    // --------------------------------------------------
    // Timing Information
    // --------------------------------------------------
    /// Duration (in seconds) of the current item
    pub duration_secs: u64,
    /// Position (in seconds) within the current item
    pub position_secs: u64,
    /// Position (as a fraction) within the current item
    pub position_fraction: f64,
    /// Rate (as a fraction) of playback speed
    pub rate_ratio: f64,
}
/// Mode of the playback
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(test, derive(serde::Serialize))]
#[allow(missing_docs)]
pub enum Mode {
    Paused,
    #[default]
    Playing,
    Stopped,
}
/// Information about the current (playing/paused) item
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(test, derive(serde::Serialize))]
#[allow(missing_docs)]
pub struct Info {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub date: String,
    pub track_number: String,
    pub track_total: String,
    pub extra: BTreeMap<String, String>,
    /// Playlist ID of the item
    pub playlist_item_id: Option<u64>,
}
impl From<StatusJSON> for Status {
    fn from(other: StatusJSON) -> Self {
        let StatusJSON {
            apiversion,
            playlist_item_id,
            information,
            duration_secs,
            is_loop_all,
            position_fraction,
            is_random,
            rate_ratio,
            is_repeat_one,
            mode,
            position_secs,
            version,
            volume_256,
        } = other;
        // convert signed time to unsigned
        let position_secs = u64::try_from(position_secs).unwrap_or(0);
        // convert InfoJSON to Info, and attach `playlist_item_id` if present
        let meta = information.map(Info::from).map(|mut meta| {
            meta.playlist_item_id = playlist_item_id.try_into().ok();
            meta
        });
        Self {
            apiversion,
            information: meta,
            is_loop_all,
            is_random,
            is_repeat_one,
            version,
            volume_percent: VolumePercent256::unchecked_to_percent(volume_256),
            duration_secs,
            position_secs,
            position_fraction,
            rate_ratio,
            mode: mode.into(),
        }
    }
}
impl From<ModeJSON> for Mode {
    fn from(value: ModeJSON) -> Self {
        match value {
            ModeJSON::Paused => Self::Paused,
            ModeJSON::Playing => Self::Playing,
            ModeJSON::Stopped => Self::Stopped,
        }
    }
}
impl From<InfoJSON> for Info {
    fn from(other: InfoJSON) -> Self {
        let MetaJSON {
            title,
            artist,
            album,
            date,
            track_number,
            track_total,
            extra,
            playlist_item_id,
        } = other.category.meta;
        Self {
            title,
            artist,
            album,
            date,
            track_number,
            track_total,
            extra,
            playlist_item_id,
        }
    }
}

#[derive(Deserialize, Debug)]
pub(crate) struct StatusJSON {
    apiversion: u32,
    #[serde(rename = "currentplid")]
    // NOTE: reports negative for `None`
    playlist_item_id: i64,
    information: Option<InfoJSON>,
    #[serde(rename = "length")]
    duration_secs: u64,
    #[serde(rename = "loop")]
    is_loop_all: bool,
    #[serde(rename = "position")]
    position_fraction: f64,
    #[serde(rename = "random")]
    is_random: bool,
    #[serde(rename = "rate")]
    rate_ratio: f64,
    #[serde(rename = "repeat")]
    is_repeat_one: bool,
    #[serde(rename = "state")]
    mode: ModeJSON,
    #[serde(rename = "time")]
    // NOTE: sometimes reports negative, but coerce to 0 for users
    position_secs: i64,
    version: String,
    /// 256-scale
    #[serde(rename = "volume")]
    volume_256: u16,
}
/// Mode of the playback
#[derive(Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
enum ModeJSON {
    Paused,
    Playing,
    Stopped,
}
#[derive(Deserialize, Debug)]
struct InfoJSON {
    category: CategoryJSON,
}
#[derive(Deserialize, Debug)]
struct CategoryJSON {
    meta: MetaJSON,
}
#[derive(Deserialize, Debug, Default, Clone, PartialEq, Eq)]
pub struct MetaJSON {
    #[serde(default)]
    title: String,
    #[serde(default)]
    artist: String,
    #[serde(default)]
    album: String,
    #[serde(default)]
    date: String,
    #[serde(default)]
    track_number: String,
    #[serde(default)]
    track_total: String,
    #[serde(flatten)]
    extra: BTreeMap<String, String>,
    #[serde(default)]
    playlist_item_id: Option<u64>,
}
