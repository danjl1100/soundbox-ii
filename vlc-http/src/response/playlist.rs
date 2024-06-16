// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Playlist response types

use serde::{Deserialize, Deserializer};

/// Playlist information
#[must_use]
#[derive(Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct Info {
    /// Items in the playlist
    pub items: Vec<Item>,
}
impl Info {
    pub(super) fn new(json: InfoJSON) -> Self {
        const GROUP_NAME_PLAYLIST: &str = "Playlist";
        let InfoJSON { groups } = json;

        // extract items from the group named "Playlist"
        let playlist_group = groups
            .into_iter()
            .find(|group| group.name == GROUP_NAME_PLAYLIST);

        let items = playlist_group
            .map(|group| group.children.into_iter().map(Item::from).collect())
            .unwrap_or_default();
        Self { items }
    }
}
impl From<ItemJSON> for Item {
    fn from(other: ItemJSON) -> Self {
        let ItemJSON {
            duration_secs,
            id,
            name,
            url,
        } = other;
        Self::new(ItemBuilder {
            duration_secs: duration_secs.try_into().ok(),
            id,
            name,
            url,
        })
    }
}
pub use item::Item;
pub(crate) use item::ItemBuilder;
mod item {
    use crate::fmt::DebugUrl;

    /// Item in the playlist (track, playlist, folder, etc.)
    #[derive(Clone, PartialEq, Eq, serde::Serialize)]
    #[allow(missing_docs)]
    pub struct Item {
        /// Duration in seconds (if known)
        duration_secs: Option<u64>,
        /// Playlist ID
        id: u64,
        name: String,
        url: DebugUrl,
    }
    impl Item {
        /// Returns the duration in seconds (if known)
        #[must_use]
        pub fn get_duration_secs(&self) -> Option<u64> {
            self.duration_secs
        }
        /// Returns the numeric identifier
        #[must_use]
        pub fn get_id(&self) -> u64 {
            self.id
        }
        /// Returns the name
        #[must_use]
        pub fn get_name(&self) -> &str {
            &self.name
        }
        /// Returns the URL
        #[must_use]
        pub fn get_url(&self) -> &url::Url {
            self.as_ref()
        }
    }
    impl AsRef<url::Url> for Item {
        fn as_ref(&self) -> &url::Url {
            self.url.as_ref()
        }
    }
    impl AsRef<DebugUrl> for Item {
        fn as_ref(&self) -> &DebugUrl {
            &self.url
        }
    }

    pub(crate) struct ItemBuilder {
        pub url: url::Url,
        pub id: u64,
        pub name: String,
        pub duration_secs: Option<u64>,
    }
    impl Item {
        pub(crate) fn new(value: ItemBuilder) -> Self {
            let ItemBuilder {
                url,
                id,
                name,
                duration_secs,
            } = value;
            Self {
                duration_secs,
                id,
                name,
                url: DebugUrl(url),
            }
        }
    }

    impl std::fmt::Debug for Item {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let Item {
                duration_secs,
                id,
                name,
                url: DebugUrl(url_raw),
            } = self;

            write!(f, r#"[{id}] "{name}""#)?;

            if let Some(duration_secs) = duration_secs {
                let duration_hour = (duration_secs / 60) / 60;
                let duration_min = (duration_secs / 60) % 60;
                let duration_sec = duration_secs % 60;
                if duration_hour == 0 {
                    write!(f, " ({duration_min}:{duration_sec:02})")?;
                } else {
                    write!(f, " ({duration_hour}:{duration_min:02}:{duration_sec:02})")?;
                }
            }
            write!(f, "  <{url_raw}>")
        }
    }
}

#[derive(Deserialize, Debug)]
pub(super) struct InfoJSON {
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
    #[serde(deserialize_with = "from_str", serialize_with = "ToString::to_string")]
    id: u64,
    name: String,
    #[serde(rename = "uri")]
    url: url::Url,
}
impl std::fmt::Debug for Info {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let Info { items } = self;
        write!(f, "Playlist items ")?;
        f.debug_list().entries(items).finish()
    }
}

fn from_str<'de, D, T>(de: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    let s = String::deserialize(de)?;
    T::from_str(&s).map_err(serde::de::Error::custom)
}
