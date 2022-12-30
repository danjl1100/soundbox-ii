// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use std::num::NonZeroUsize;

use super::{playback_mode, ConverterIterator, LowAction};
use crate::command::LowCommand;
use crate::controller::RepeatMode;
use crate::vlc_responses::{ItemsFmt, PlaybackStatus, PlaylistInfo, PlaylistItem};

// NOTE needs to be located "exactly here", for relative use in sub-modules' tests
#[cfg(test)]
macro_rules! items {
    ($($url:expr),* ; ..$remaining_urls:expr) => {
        {
            let mut front = items!($($url),*);
            let mut back = items!(@slice $remaining_urls);
            let front_len = front.len();
            for (back_idx, back_item) in back.iter_mut().enumerate() {
                back_item.id = (front_len + back_idx).to_string();
            }
            front.append(&mut back);
            front
        }
    };
    ($($url:expr),* $(,)?) => {
        {
            let item_urls = &[ $($url),* ];
            items!(@slice item_urls)
        }
    };
    (@slice $urls:expr) => {
        $crate::controller::high_converter::playlist_set::tests::
            playlist_items_with_urls($urls)
    };
    ($($id:expr => $url:expr),* $(,)?) => {
        {
            let item_ids_urls: &[(usize, &str)] = &[ $( ($id, $url) ),* ];
            $crate::controller::high_converter::playlist_set::tests::
                playlist_items_with_ids_urls(item_ids_urls.iter().copied())
        }
    };
}
#[cfg(test)]
mod tests;

mod enqueue;
mod remove;

use index_item::IndexItem;
mod index_item;

#[derive(Debug)]
pub struct Command {
    pub urls: Vec<url::Url>,
    /// See documentation for [`crate::command::HighCommand`]
    pub max_history_count: NonZeroUsize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Converter {
    converter_mode: playback_mode::Converter,
    // previously-accepted comparison point
    accepted_comparison_start: Option<usize>,
}
impl Converter {
    pub fn new() -> Self {
        Self {
            converter_mode: playback_mode::Converter,
            accepted_comparison_start: None,
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
        let result = self.next_playlist_set((status, playlist), command);
        {
            // DEBUG
            let current_id = status.information.as_ref().and_then(|i| i.playlist_item_id);
            println!(
                "DEBUG RESULT {result:?} for current_id {current_id:?}, playlist {items:#?}",
                result = ResultFmt(&result),
                items = ItemsFmt(&playlist.items),
            );
        }
        result
    }
}
impl Converter {
    fn next_playlist_set(
        &mut self,
        (status, playlist): (&PlaybackStatus, &PlaylistInfo),
        command: &Command,
    ) -> Result<(), LowAction> {
        let items = &playlist.items;
        let current_id = status.information.as_ref().and_then(|i| i.playlist_item_id);
        let comparison_start = if let Some(accepted) = self.accepted_comparison_start {
            accepted
        } else {
            // [STEP 1] remove prior to current, to match `max_history_count`
            let current_item = IndexItem::new(current_id, items, command);
            let current_index = current_item.map(|c| (c.ty, c.index));
            Self::check_remove_first(items, current_index, command)?;
            current_item.map_or_else(|| items.len(), |c| c.get_comparison_start())
        };
        // shrink `accepted` if items.len has shrunk
        let comparison_start = comparison_start.min(items.len());
        self.accepted_comparison_start = Some(comparison_start);
        let comparison_items = &items[comparison_start..];
        // - only delete incorrect-current items AFTER the correct items are fully staged
        //    (pros: allow VLC to catch-up on file metadata loading)
        //    (cons: delays the change-over when many-many items need adding)
        // [STEP 2] add items after the "current" item (skips any items already present and in-order)
        let desired_id = Self::enqueue_items(comparison_items, command)?;
        // [STEP 3] delete items after the "current" item (to leave only desired items remaining)
        Self::delete_items(comparison_items, command)?;
        // NOTE: when an item is already playing, the desired item *SHALL NOT* be `PlaylistPlay`ed here,
        //  in order to remain robust against unforseen external playlist edits
        let current_item = IndexItem::new(current_id, items, command);
        match (current_item, desired_id) {
            (None, Some(desired_id)) => {
                let item_id = Some(desired_id);
                Err(LowCommand::PlaylistPlay { item_id }.into())
            }
            _ => Ok(()),
        }
    }
}

/// Debug-coercion for `PlaylistAdd` urls to be literal strings
#[derive(PartialEq)]
struct ResultFmt<'a>(&'a Result<(), LowAction>);
impl<'a> std::fmt::Debug for ResultFmt<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Err(LowAction::Command(LowCommand::PlaylistAdd { url })) => f
                .debug_struct("PlaylistAdd")
                .field("url", &url.to_string())
                .finish(),
            inner => write!(f, "{inner:?}"),
        }
    }
}
