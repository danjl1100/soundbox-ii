//! Response data types from the VLC HTTP endpoint

#![allow(clippy::module_name_repetitions)]

pub use playback::{PlaybackInfo, PlaybackState, PlaybackStatus};
mod playback {
    use crate::command;

    use serde::Deserialize;
    use std::collections::HashMap;
    use std::convert::TryInto;

    /// Status of the current playback
    #[derive(Debug)]
    #[allow(missing_docs)]
    pub struct PlaybackStatus {
        /// version of the VLC-HTTP interface api
        pub apiversion: u32,
        /// Information about the current item
        pub information: Option<PlaybackInfo>,
        /// Duration of the current song in seconds
        pub duration: u64,
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
                playlist_item_id,
                information,
                duration,
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
            let meta = information.map(PlaybackInfo::from).map(|mut meta| {
                meta.playlist_item_id = playlist_item_id.try_into().ok();
                meta
            });
            Self {
                apiversion,
                information: meta,
                duration,
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
        #[serde(rename = "currentplid")]
        playlist_item_id: i64,
        information: Option<PlaybackInfoJSON>,
        #[serde(rename = "length")]
        duration: u64,
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
        /// Playlist ID of the item
        #[serde(skip)]
        pub playlist_item_id: Option<u64>,
    }
    impl From<PlaybackInfoJSON> for PlaybackInfo {
        fn from(other: PlaybackInfoJSON) -> Self {
            other.category.meta
        }
    }
}

pub use playlist::{PlaylistInfo, PlaylistItem};
mod playlist {
    use serde::Deserialize;
    use std::convert::TryInto;

    /// Playlist information
    #[allow(missing_docs)]
    pub struct PlaylistInfo {
        pub items: Vec<PlaylistItem>,
    }
    impl PlaylistInfo {
        pub(crate) fn from_slice(bytes: &[u8]) -> Result<Self, serde_json::Error> {
            let json_struct: PlaylistRootJSON = serde_json::from_slice(bytes)?;
            Ok(Self::from(json_struct))
        }
    }
    impl From<PlaylistRootJSON> for PlaylistInfo {
        fn from(other: PlaylistRootJSON) -> Self {
            const GROUP_NAME_PLAYLIST: &str = "Playlist";
            let PlaylistRootJSON { groups } = other;
            let playlist_group = groups
                .into_iter()
                .find(|group| group.name == GROUP_NAME_PLAYLIST);
            let items = playlist_group.map_or_else(Vec::new, |group| {
                group.children.into_iter().map(PlaylistItem::from).collect()
            });
            Self { items }
        }
    }

    #[derive(Deserialize, Debug)]
    struct PlaylistRootJSON {
        #[serde(rename = "children")]
        groups: Vec<GroupNodeJSON>,
    }
    #[derive(Deserialize, Debug)]
    struct GroupNodeJSON {
        name: String,
        children: Vec<PlaylistItemJSON>,
    }
    #[derive(Deserialize, Debug)]
    struct PlaylistItemJSON {
        duration: i64,
        id: String,
        name: String,
        uri: String,
    }
    /// Item in the playlist (track, playlist, folder, etc.)
    #[derive(Debug)]
    #[allow(missing_docs)]
    pub struct PlaylistItem {
        /// Duration of the current song in seconds
        pub duration: Option<u64>,
        /// Playlist ID
        pub id: String,
        pub name: String,
        pub uri: String,
    }
    impl From<PlaylistItemJSON> for PlaylistItem {
        fn from(other: PlaylistItemJSON) -> Self {
            let PlaylistItemJSON {
                duration,
                id,
                name,
                uri,
            } = other;
            Self {
                duration: duration.try_into().ok(),
                id,
                name,
                uri,
            }
        }
    }
    impl std::fmt::Debug for PlaylistInfo {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            let PlaylistInfo { items } = self;
            writeln!(f, "Playlist items [")?;
            for item in items {
                let PlaylistItem {
                    duration,
                    id,
                    name,
                    uri,
                } = item;
                //
                write!(f, r#"   [{id}] "{name}""#, id = id, name = name)?;
                //
                if let Some(duration) = duration {
                    let duration_hour = (duration / 60) / 60;
                    let duration_min = (duration / 60) % 60;
                    let duration_sec = duration % 60;
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
