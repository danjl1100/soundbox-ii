// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use super::{playback_mode, ConverterIterator, LowAction};
use crate::controller::{PlaybackStatus, PlaylistInfo, RepeatMode};

#[cfg(test)]
macro_rules! items {
    ($($url:expr),* $(,)?) => {
        {
            let item_urls = &[ $($url),* ];
            $crate::controller::high_converter::playlist_set::tests::
                playlist_items_with_urls(item_urls)
        }
    };
    ($($id:expr => $url:expr),* $(,)?) => {
        {
            let item_ids_urls: &[(usize, &str)] = &[ $( ($id, $url) ),* ];
            $crate::controller::high_converter::playlist_set::tests::
                playlist_items_with_ids_urls(item_ids_urls.iter().copied())
        }
    };
}

mod previous;

mod current;

mod next;

#[derive(Debug)]
pub struct Command {
    pub current_or_past_url: url::Url,
    pub next_urls: Vec<url::Url>,
    pub max_history_count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ComparisonStart {
    index: usize,
    skip_current: bool,
}
impl ComparisonStart {
    fn at(index: usize) -> Self {
        Self {
            index,
            skip_current: true,
        }
    }
    fn include_current(mut self) -> Self {
        self.skip_current = false;
        self
    }
    fn iter_current<T>(&self, current: T) -> impl Iterator<Item = T> {
        std::iter::once(current).skip(if self.skip_current { 1 } else { 0 })
    }
    fn iter_source_urls<'a>(&self, command: &'a Command) -> impl Iterator<Item = &'a url::Url> {
        let Command {
            current_or_past_url,
            next_urls,
            ..
        } = command;
        self.iter_current(current_or_past_url)
            .chain(next_urls.iter())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Converter {
    converter_mode: playback_mode::Converter,
    // marker to only allow sending "play" ONCE (in case it fails, for a nonexistent file)
    play_command: Option<()>,
}
impl Converter {
    pub fn new() -> Self {
        Self {
            converter_mode: playback_mode::Converter,
            play_command: Some(()),
        }
    }
}
impl<'a> ConverterIterator<'a> for Converter {
    type Status = (&'a PlaybackStatus, &'a PlaylistInfo);
    type Command = Command;
    fn next(
        &mut self,
        (status, playlist): Self::Status,
        command: &Command,
    ) -> Result<(), LowAction> {
        // [STEP 0] ensure playback mode is correct for in-order consumption
        self.converter_mode.next(
            status,
            &playback_mode::Command {
                repeat: RepeatMode::Off,
                random: false,
            },
        )?;
        let items = &playlist.items;
        let current_id = status.information.as_ref().and_then(|i| i.playlist_item_id);
        let current_index_item = current_id.and_then(|id| {
            let current_id_str = id.to_string();
            items
                .iter()
                .enumerate()
                .find(|(_, item)| (item.id == current_id_str))
        });
        // [STEP 1] remove prior to 'current_or_past_url', to match `max_history_count`
        Self::remove_previous_items(items, current_index_item, command)?;
        // [STEP 2] set current item
        let comparison_start = self.prep_comparison_start(items, current_index_item, command)?;
        //
        for (index, item) in items.iter().enumerate() {
            let url = &item.url;
            println!("DEBUG ITEM #{index}: {url}");
        }
        // [STEP 3] compare next_urls to items, starting with index from step 2
        Self::compare(items, comparison_start, command)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::vlc_responses::PlaylistItem;

    pub(super) fn playlist_items_with_urls(urls: &[&str]) -> Vec<PlaylistItem> {
        playlist_items_with_ids_urls(urls.iter().copied().enumerate())
    }
    pub(super) fn playlist_items_with_ids_urls<'a, T, U>(ids_urls: T) -> Vec<PlaylistItem>
    where
        T: IntoIterator<Item = (U, &'a str)>,
        U: ToString,
    {
        ids_urls
            .into_iter()
            .map(|(id, url)| PlaylistItem {
                duration_secs: None,
                id: id.to_string(),
                name: String::default(),
                url: file_url(url).to_string(),
            })
            .collect()
    }
    pub(super) fn file_url(s: &str) -> url::Url {
        url::Url::parse(&format!("file:///{s}")).expect("url")
    }
    pub(super) fn calc_current_item_index<'a>(
        items: &'a [PlaylistItem],
        current_url: &Option<String>,
    ) -> Option<(usize, &'a PlaylistItem)> {
        current_url.as_ref().and_then(|current_url| {
            items
                .iter()
                .enumerate()
                .find(|(_, item)| (item.url == *current_url))
        })
    }
}
