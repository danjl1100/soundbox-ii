// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use super::{playback_mode, ConverterIterator, LowAction};
use crate::controller::{LowCommand, PlaybackStatus, PlaylistInfo, RepeatMode};
use crate::vlc_responses::PlaylistItem;

#[derive(Debug)]
pub struct Command {
    pub current_or_past_url: url::Url,
    pub next_urls: Vec<url::Url>,
    pub max_history_count: usize,
}

#[derive(Clone, Copy)]
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

#[derive(Debug)]
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
        Self::remove_prior_items(items, current_index_item, command)?;
        // [STEP 2] set current item
        let comparison_start = self.set_current_item(items, current_index_item, command)?;
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
impl Converter {
    fn remove_prior_items(
        items: &[PlaylistItem],
        current_index_item: Option<(usize, &PlaylistItem)>,
        command: &Command,
    ) -> Result<(), LowCommand> {
        let Command {
            max_history_count, ..
        } = command;
        let current_index = current_index_item.map(|(index, _)| index);
        // [1] remove prior to 'current_or_past_url', to match `max_history_count`
        if let Some(PlaylistItem { id: first_id, .. }) = items.first() {
            let remove_first = match current_index {
                Some(current_index) if current_index > *max_history_count => true,
                None if items.len() > *max_history_count => true,
                _ => false, // history length within bounds
            };
            if remove_first {
                let item_id = first_id.clone();
                Err(LowCommand::PlaylistDelete { item_id })?;
            }
        }
        Ok(())
    }
    fn set_current_item(
        &mut self,
        items: &[PlaylistItem],
        current_index_item: Option<(usize, &PlaylistItem)>,
        command: &Command,
    ) -> Result<ComparisonStart, LowCommand> {
        let current_or_past_url_str = command.current_or_past_url.to_string();
        let current_index = current_index_item.map(|(index, _)| index);
        let previous_item = current_index
            .and_then(|current| current.checked_sub(1))
            .and_then(|prev| items.get(prev));
        // TODO deleteme, or rename to `next_or_last_item`
        // let end_item = current_index
        //     .and_then(|current| items.get(current + 1))
        //     .or_else(|| items.last());
        let end_item = items.last();
        // [2] set current item
        match (current_index_item, previous_item, end_item) {
            (Some((current_index, current)), _, _) if current.url == current_or_past_url_str => {
                // start comparison after current
                Ok(ComparisonStart::at(current_index + 1))
            }
            (Some((current_index, _)), Some(prev), _) if prev.url == current_or_past_url_str => {
                // start comparison at current
                Ok(ComparisonStart::at(current_index))
            }
            (Some((_, wrong_current_item)), _, _) => {
                // current item is wrong URI, delete it
                let item_id = wrong_current_item.id.clone();
                Err(LowCommand::PlaylistDelete { item_id })
            }
            (None, _, Some(last)) if last.url == current_or_past_url_str => {
                if self.play_command.take().is_some() {
                    // start last item, it's a match!
                    let item_id = Some(last.id.clone());
                    Err(LowCommand::PlaylistPlay { item_id })
                } else {
                    // failed to start, attempt to continue setting playlist
                    Ok(ComparisonStart::at(items.len()))
                }
            }
            (None, _, _) => Ok(ComparisonStart::at(items.len()).include_current()), // no current item, start at end
        }
    }
    fn compare(
        items: &[PlaylistItem],
        start: ComparisonStart,
        command: &Command,
    ) -> Result<(), LowCommand> {
        // [3] compare next_urls to items, starting with index from previous step
        let mut items = items.iter().skip(start.index);
        let source_urls = start.iter_source_urls(command);
        for next_url in source_urls {
            match items.next() {
                Some(item) if item.url == *next_url.to_string() => continue,
                Some(wrong_item) => {
                    let item_id = wrong_item.id.clone();
                    Err(LowCommand::PlaylistDelete { item_id })?;
                }
                None => {
                    let url = next_url.clone();
                    Err(LowCommand::PlaylistAdd { url })?;
                }
            }
        }
        Ok(())
    }
}
