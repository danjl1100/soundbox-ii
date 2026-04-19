// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use crate::volume_256::VolumePercent256;
use serde::Deserialize;
use std::collections::BTreeMap;

/// Status of the current playback
#[must_use]
#[derive(Clone, PartialEq, serde::Serialize)]
#[non_exhaustive]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize)]
pub enum Mode {
    Paused,
    #[default]
    Playing,
    Stopped,
}
/// Information about the current (playing/paused) item
#[derive(Default, Clone, PartialEq, Eq, serde::Serialize)]
#[non_exhaustive]
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
macro_rules! debug_field_if {
    (
        $debug:ident, |$value:ident| $condition:expr => { $($name:ident),+ $(,)? }
    ) => {
        $(
            {
                let $value = $name;
                if $condition {
                    $debug .field(stringify!($name), $name);
                }
            }
        )+
    };
}
impl std::fmt::Debug for Info {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            title,
            artist,
            album,
            date,
            track_number,
            track_total,
            extra,
            playlist_item_id,
        } = self;

        let mut debug = f.debug_struct("Info");

        debug_field_if!(debug, |s| !s.is_empty() => {
            title,
            artist,
            album,
            date,
            track_number,
            track_total,
            extra,
        });

        if let Some(playlist_item_id) = playlist_item_id {
            debug.field("playlist_item_id", playlist_item_id);
        }

        debug.finish()
    }
}
impl std::fmt::Debug for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        const KNOWN_VERSIONS: &[&str] = &["3.0.20 Vetinari"];

        let Self {
            apiversion,
            information,
            is_loop_all,
            is_random,
            is_repeat_one,
            version,
            volume_percent,
            mode,
            duration_secs,
            position_secs,     // OK if 0
            position_fraction, // OK if 0.0
            rate_ratio,
        } = self;

        let mut debug = f.debug_struct("Status");

        debug.field("api", apiversion);

        if let Some(info) = information {
            debug.field("info", info);
        }

        {
            use std::fmt::Write as _;

            let kind_loop = is_loop_all.then_some("all");
            let kind_repeat = is_repeat_one.then_some("one");
            let kind_random = is_random.then_some("random");
            let mut kinds = String::new();
            for kind in [kind_loop, kind_repeat, kind_random].into_iter().flatten() {
                let separator = if kinds.is_empty() { "" } else { " " };
                write!(&mut kinds, "{separator}{kind}").expect("infallible");
            }
            debug.field("ordering", &kinds);
        }

        debug_field_if!(debug, |s| !KNOWN_VERSIONS.contains(&&**s) => {
            version,
        });

        debug.field("vol", volume_percent);

        debug_field_if!(debug, |v| *v != 0 => {
            duration_secs,
        });

        {
            let rate = if (rate_ratio - 1.0).abs() < 1e-8 {
                String::new()
            } else {
                format!(" @ {rate_ratio}x")
            };

            debug.field(
                "position",
                &format!("{mode:?} {position_secs}s, {position_fraction:.2}{rate}"),
            );
        }

        debug.finish()
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
