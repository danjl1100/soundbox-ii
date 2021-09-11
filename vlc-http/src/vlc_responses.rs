//! Response data types from the VLC HTTP endpoint

#![allow(clippy::module_name_repetitions)]

pub use playback::{PlaybackInfo, PlaybackState, PlaybackStatus};
mod playback {
    use crate::command;
    use shared::Time;

    use serde::Deserialize;
    use std::collections::HashMap;
    use std::convert::TryInto;

    /// Status of the current playback
    #[derive(Debug, Clone, PartialEq, Eq)]
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
        pub position: Position,
        /// True if playlist-randomize is enabled
        pub is_random: bool,
        /// Playback rate (unit scale)
        pub rate: Rate,
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
        /// Received Time
        pub received_time: Time,
    }
    impl PlaybackStatus {
        pub(crate) fn from_slice(
            bytes: &[u8],
            received_time: Time,
        ) -> Result<Self, serde_json::Error> {
            let json_struct: PlaybackStatusJSON = serde_json::from_slice(bytes)?;
            Ok(Self::from((json_struct, received_time)))
        }
    }
    impl From<(PlaybackStatusJSON, Time)> for PlaybackStatus {
        fn from((other, received_time): (PlaybackStatusJSON, Time)) -> Self {
            use std::convert::TryFrom;
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
            // convert signed time to unsigned
            let time = u64::try_from(time.max(0)).expect("non-negative i64 can convert to u64");
            // convert PlaybackInfoJSON to PlaybackInfo, and attach `playlist_item_id` if present
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
                received_time,
            }
        }
    }
    macro_rules! cheap_float_eq {
        (
            $(
                #[$($attr:meta)*]
                $vis:vis struct $name:ident (pub $float_ty:ty );
            )+
        ) => {
            $(
                $(#[$attr])*
                #[derive(PartialOrd, Deserialize)]
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
        #[derive(Debug, Default, Clone, Copy)]
        pub struct Position(pub f64);

        #[derive(Debug, Default, Clone, Copy)]
        pub struct Rate(pub f64);
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
        position: Position, //f64,
        #[serde(rename = "random")]
        is_random: bool,
        rate: Rate, //f64,
        #[serde(rename = "repeat")]
        is_repeat: bool,
        state: PlaybackState,
        time: i64,
        version: String,
        /// 256-scale
        volume: u32,
    }
    /// Mode of the playback
    #[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
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
    #[derive(Deserialize, Debug, Default, Clone, PartialEq, Eq)]
    #[allow(missing_docs)]
    pub struct PlaybackInfo {
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
    impl From<PlaybackInfoJSON> for PlaybackInfo {
        fn from(other: PlaybackInfoJSON) -> Self {
            other.category.meta
        }
    }
}

pub use playlist::{PlaylistInfo, PlaylistItem};
mod playlist {
    use serde::Deserialize;
    use shared::Time;
    use std::convert::TryInto;

    /// Playlist information
    #[derive(Clone, PartialEq, Eq)]
    pub struct PlaylistInfo {
        /// Items in the playlist
        pub items: Vec<PlaylistItem>,
        /// Received Time
        pub received_time: Time,
    }
    impl PlaylistInfo {
        pub(crate) fn from_slice(
            bytes: &[u8],
            received_time: Time,
        ) -> Result<Self, serde_json::Error> {
            let json_struct: PlaylistRootJSON = serde_json::from_slice(bytes)?;
            Ok(Self::from((json_struct, received_time)))
        }
    }
    impl From<(PlaylistRootJSON, Time)> for PlaylistInfo {
        fn from((other, received_time): (PlaylistRootJSON, Time)) -> Self {
            const GROUP_NAME_PLAYLIST: &str = "Playlist";
            let PlaylistRootJSON { groups } = other;
            let playlist_group = groups
                .into_iter()
                .find(|group| group.name == GROUP_NAME_PLAYLIST);
            let items = playlist_group.map_or_else(Vec::new, |group| {
                group.children.into_iter().map(PlaylistItem::from).collect()
            });
            Self {
                items,
                received_time,
            }
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
    #[derive(Debug, Clone, PartialEq, Eq)]
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
            let PlaylistInfo {
                items,
                received_time,
            } = self;
            writeln!(f, "Playlist items @ {:?} [", received_time)?;
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

mod external_conversions {
    use super::{PlaybackInfo, PlaybackState, PlaybackStatus};
    use shared::Time;
    impl PlaybackStatus {
        /// Clones the [`PlaybackStatus`] to a [`shared::PlaybackStatus`], modifying all
        /// time values as appropriate for the given now [`Time`].
        //TODO: eliminate needless clone prior to serializing
        //  from: `vlc_http` type --clone--> `shared` type --copy into--> serde string
        //  to:   `vlc_http` type --reference--> shared reference type --copy into --> serde string
        pub fn clone_to_shared(&self, now: Time) -> shared::PlaybackStatus {
            let Self {
                information,
                duration,
                position,
                rate,
                state,
                time,
                volume_percent,
                received_time,
                ..
            } = self;
            let duration = *duration;
            let position = f64::from(*position);
            let time = *time;
            let (position, time) = if *state == PlaybackState::Playing {
                // calculate age of the information
                let age = now - *received_time;
                #[allow(clippy::cast_precision_loss)]
                let age_seconds_float = (age.num_milliseconds() as f64) / 1000.0;
                #[allow(clippy::cast_possible_truncation)]
                #[allow(clippy::cast_sign_loss)]
                let age_seconds = age_seconds_float.ceil().abs() as u64;
                //
                let position = {
                    #[allow(clippy::cast_precision_loss)]
                    let duration = duration as f64;
                    let stored = position;
                    // predict
                    stored + (age_seconds_float / duration)
                };
                let time = {
                    let stored = time;
                    let predict = stored + age_seconds;
                    predict.min(duration)
                };
                (position, time)
            } else {
                (position, time)
            };
            shared::PlaybackStatus {
                information: information.as_ref().map(PlaybackInfo::clone_to_shared),
                duration,
                position,
                rate: (*rate).into(),
                state: (*state).into(),
                time,
                volume_percent: *volume_percent,
            }
        }
    }
    impl From<PlaybackState> for shared::PlaybackState {
        fn from(other: PlaybackState) -> Self {
            match other {
                PlaybackState::Paused => Self::Paused,
                PlaybackState::Playing => Self::Playing,
                PlaybackState::Stopped => Self::Stopped,
            }
        }
    }
    impl PlaybackInfo {
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
    #[cfg(test)]
    mod for_tests {
        use super::{super::PlaylistInfo, PlaybackStatus};
        fn fake_received_time() -> shared::Time {
            shared::time_from_secs(0)
        }
        impl Default for PlaybackStatus {
            fn default() -> Self {
                PlaybackStatus {
                    apiversion: Default::default(),
                    information: Default::default(),
                    duration: Default::default(),
                    is_loop: Default::default(),
                    position: Default::default(),
                    is_random: Default::default(),
                    rate: Default::default(),
                    is_repeat: Default::default(),
                    state: Default::default(),
                    time: Default::default(),
                    version: Default::default(),
                    volume_percent: Default::default(),
                    received_time: fake_received_time(),
                }
            }
        }
        impl Default for super::PlaybackState {
            fn default() -> Self {
                Self::Playing
            }
        }
        //
        impl Default for PlaylistInfo {
            fn default() -> Self {
                PlaylistInfo {
                    items: vec![],
                    received_time: fake_received_time(),
                }
            }
        }
    }
}
