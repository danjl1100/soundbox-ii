// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Playlist response types

use serde::Deserialize;

/// Playlist information
#[must_use]
#[derive(Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct Info {
    /// Items in the playlist
    pub items: Vec<Item>,
}
/// Item in the playlist (track, playlist, folder, etc.)
#[derive(Clone, PartialEq, Eq, serde::Serialize)]
#[allow(missing_docs)]
pub struct Item {
    /// Duration of the current song in seconds
    pub duration_secs: Option<u64>,
    /// Playlist ID
    pub id: String,
    pub name: String,
    pub url: url::Url,
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
        Self {
            duration_secs: duration_secs.try_into().ok(),
            id,
            name,
            url,
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
    id: String,
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
impl std::fmt::Debug for Item {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Item {
            duration_secs,
            id,
            name,
            url,
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
        write!(f, "  <{url}>")
    }
}
