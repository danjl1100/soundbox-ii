//! Response data types from the VLC HTTP endpoint

pub use playback::Info as PlaybackInfo;
pub use playback::Status as PlaybackStatus;
pub use shared::{PlaybackState, PlaybackTiming};
mod playback {
    use crate::command;
    use shared::{PlaybackTiming, PositionFraction, RateRatio, Time};

    use serde::Deserialize;
    use std::collections::HashMap;
    use std::convert::TryInto;

    /// Status of the current playback
    #[must_use]
    #[derive(Debug, Clone, PartialEq, Eq)]
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
        /// Playback Timing information
        pub timing: shared::PlaybackTiming,
        /// Received Time
        pub received_time: Time,
    }
    impl Status {
        pub(crate) fn from_slice(
            bytes: &[u8],
            received_time: Time,
        ) -> Result<Self, serde_json::Error> {
            let json_struct: StatusJSON = serde_json::from_slice(bytes)?;
            Ok(Self::from((json_struct, received_time)))
        }
    }
    impl From<(StatusJSON, Time)> for Status {
        fn from((other, received_time): (StatusJSON, Time)) -> Self {
            use std::convert::TryFrom;
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
                state,
                position_secs,
                version,
                volume,
            } = other;
            // convert signed time to unsigned
            let position_secs = u64::try_from(position_secs).unwrap_or(0);
            // convert InfoJSON to Info, and attach `playlist_item_id` if present
            let meta = information.map(Info::from).map(|mut meta| {
                meta.playlist_item_id = playlist_item_id.try_into().ok();
                meta
            });
            let state = shared::PlaybackState::from(state);
            Self {
                apiversion,
                information: meta,
                is_loop_all,
                is_random,
                is_repeat_one,
                version,
                volume_percent: command::decode_volume_to_percent(volume),
                timing: PlaybackTiming {
                    duration_secs,
                    position_fraction,
                    rate_ratio,
                    state,
                    position_secs,
                },
                received_time,
            }
        }
    }

    #[derive(Deserialize, Debug)]
    pub(crate) struct StatusJSON {
        apiversion: u32,
        #[serde(rename = "currentplid")]
        playlist_item_id: i64,
        information: Option<InfoJSON>,
        #[serde(rename = "length")]
        duration_secs: u64,
        #[serde(rename = "loop")]
        is_loop_all: bool,
        #[serde(rename = "position")]
        position_fraction: PositionFraction, //f64,
        #[serde(rename = "random")]
        is_random: bool,
        #[serde(rename = "rate")]
        rate_ratio: RateRatio, //f64,
        #[serde(rename = "repeat")]
        is_repeat_one: bool,
        state: StateJSON,
        #[serde(rename = "time")]
        position_secs: i64,
        version: String,
        /// 256-scale
        volume: u32,
    }
    /// Mode of the playback
    #[derive(Deserialize, Debug)]
    #[serde(rename_all = "lowercase")]
    pub(crate) enum StateJSON {
        /// Paused
        Paused,
        /// Playing
        Playing,
        /// Stopped
        Stopped,
    }
    #[derive(Deserialize, Debug)]
    pub(crate) struct InfoJSON {
        category: CategoryJSON,
    }
    #[derive(Deserialize, Debug)]
    pub(crate) struct CategoryJSON {
        meta: Info,
    }
    /// Information about the current (playing/paused) item
    #[derive(Deserialize, Debug, Default, Clone, PartialEq, Eq)]
    #[allow(missing_docs)]
    pub struct Info {
        #[serde(default)]
        pub title: String,
        #[serde(default)]
        pub artist: String,
        #[serde(default)]
        pub album: String,
        #[serde(default)]
        pub date: String,
        #[serde(default)]
        pub track_number: String,
        #[serde(default)]
        pub track_total: String,
        #[serde(flatten)]
        pub extra: HashMap<String, String>,
        /// Playlist ID of the item
        #[serde(default)]
        pub playlist_item_id: Option<u64>,
    }
    impl From<InfoJSON> for Info {
        fn from(other: InfoJSON) -> Self {
            other.category.meta
        }
    }
}

pub use playlist::Info as PlaylistInfo;
pub use playlist::Item as PlaylistItem;
mod playlist {
    use serde::Deserialize;
    use shared::Time;
    use std::convert::TryInto;

    /// Playlist information
    #[must_use]
    #[derive(Clone, PartialEq, Eq)]
    pub struct Info {
        /// Items in the playlist
        pub items: Vec<Item>,
        /// Received Time
        pub received_time: Time,
    }
    impl Info {
        pub(crate) fn from_slice(
            bytes: &[u8],
            received_time: Time,
        ) -> Result<Self, serde_json::Error> {
            let json_struct: RootJSON = serde_json::from_slice(bytes)?;
            Ok(Self::from((json_struct, received_time)))
        }
    }
    impl From<(RootJSON, Time)> for Info {
        fn from((other, received_time): (RootJSON, Time)) -> Self {
            const GROUP_NAME_PLAYLIST: &str = "Playlist";
            let RootJSON { groups } = other;
            let playlist_group = groups
                .into_iter()
                .find(|group| group.name == GROUP_NAME_PLAYLIST);
            let items = playlist_group.map_or_else(Vec::new, |group| {
                group.children.into_iter().map(Item::from).collect()
            });
            Self {
                items,
                received_time,
            }
        }
    }

    #[derive(Deserialize, Debug)]
    struct RootJSON {
        #[serde(rename = "children")]
        groups: Vec<GroupNodeJSON>,
    }
    #[derive(Deserialize, Debug)]
    struct GroupNodeJSON {
        name: String,
        children: Vec<ItemJSON>,
    }
    #[derive(Deserialize, Debug)]
    struct ItemJSON {
        #[serde(rename = "duration")]
        duration_secs: i64,
        id: String,
        name: String,
        uri: String,
    }
    /// Item in the playlist (track, playlist, folder, etc.)
    #[derive(Debug, Clone, PartialEq, Eq)]
    #[allow(missing_docs)]
    pub struct Item {
        /// Duration of the current song in seconds
        pub duration_secs: Option<u64>,
        /// Playlist ID
        pub id: String,
        pub name: String,
        pub uri: String,
    }
    impl From<ItemJSON> for Item {
        fn from(other: ItemJSON) -> Self {
            let ItemJSON {
                duration_secs,
                id,
                name,
                uri,
            } = other;
            Self {
                duration_secs: duration_secs.try_into().ok(),
                id,
                name,
                uri,
            }
        }
    }
    impl std::fmt::Debug for Info {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            let Info {
                items,
                received_time,
            } = self;
            writeln!(f, "Playlist items @ {:?} [", received_time)?;
            for item in items {
                let Item {
                    duration_secs,
                    id,
                    name,
                    uri,
                } = item;
                //
                write!(f, r#"   [{id}] "{name}""#, id = id, name = name)?;
                //
                if let Some(duration_secs) = duration_secs {
                    let duration_hour = (duration_secs / 60) / 60;
                    let duration_min = (duration_secs / 60) % 60;
                    let duration_sec = duration_secs % 60;
                    if duration_hour == 0 {
                        write!(f, " ({}:{:02})", duration_min, duration_sec)?;
                    } else {
                        write!(
                            f,
                            " ({}:{:02}:{:02})",
                            duration_hour, duration_min, duration_sec
                        )?;
                    }
                }
                writeln!(f, "\n\t{uri}", uri = uri)?;
            }
            writeln!(f, "]")
        }
    }
}

mod external_conversions {
    use super::playback;
    #[allow(unused)]
    use super::{PlaybackInfo, PlaybackStatus}; // for doc-comments
    use shared::Time;
    impl playback::Status {
        /// Clones the [`PlaybackStatus`] to a [`shared::PlaybackStatus`], modifying all
        /// time values as appropriate for the given now [`Time`].
        //TODO: eliminate needless clone prior to serializing
        //  from: `vlc_http` type --clone--> `shared` type --copy into--> serde string
        //  to:   `vlc_http` type --reference--> shared reference type --copy into --> serde string
        pub fn clone_to_shared(&self, now: Time) -> shared::PlaybackStatus {
            let Self {
                information,
                volume_percent,
                timing,
                ..
            } = self;
            let timing = timing.predict_change(now - self.received_time);
            shared::PlaybackStatus {
                information: information.as_ref().map(playback::Info::clone_to_shared),
                volume_percent: *volume_percent,
                timing,
            }
        }
    }
    impl From<playback::StateJSON> for shared::PlaybackState {
        fn from(other: playback::StateJSON) -> Self {
            match other {
                playback::StateJSON::Paused => Self::Paused,
                playback::StateJSON::Playing => Self::Playing,
                playback::StateJSON::Stopped => Self::Stopped,
            }
        }
    }
    impl playback::Info {
        /// Clones the [`PlaybackInfo`] to a [`shared::PlaybackInfo`]
        //TODO: eliminate needless clone prior to serializing
        //  from: `vlc_http` type --clone--> `shared` type       --copy into--> serde string
        //  to:   `vlc_http` type --ref--> shared reference type --copy into--> serde string
        pub fn clone_to_shared(&self) -> shared::PlaybackInfo {
            let Self {
                title,
                artist,
                album,
                date,
                track_number,
                track_total,
                playlist_item_id,
                ..
            } = self;
            shared::PlaybackInfo {
                title: title.clone(),
                artist: artist.clone(),
                album: album.clone(),
                date: date.clone(),
                track_number: track_number.clone(),
                track_total: track_total.clone(),
                playlist_item_id: *playlist_item_id,
            }
        }
    }
}
#[cfg(test)]
mod for_tests {
    use super::{playback, playlist};
    fn fake_received_time() -> shared::Time {
        shared::time_from_secs(0)
    }
    impl Default for playback::Status {
        fn default() -> Self {
            playback::Status {
                apiversion: Default::default(),
                information: Option::default(),
                is_loop_all: Default::default(),
                is_random: Default::default(),
                is_repeat_one: Default::default(),
                version: String::default(),
                volume_percent: Default::default(),
                timing: shared::PlaybackTiming::default(),
                received_time: fake_received_time(),
            }
        }
    }
    //
    impl Default for playlist::Info {
        fn default() -> Self {
            playlist::Info {
                items: vec![],
                received_time: fake_received_time(),
            }
        }
    }
}
