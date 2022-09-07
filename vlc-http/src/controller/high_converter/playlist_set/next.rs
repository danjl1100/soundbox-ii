// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use super::{Command, ComparisonStart, Converter};
use crate::controller::LowCommand;
use crate::vlc_responses::PlaylistItem;

impl Converter {
    pub(super) fn compare(
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

#[cfg(test)]
mod tests {
    use super::super::{tests::file_url, Command, ComparisonStart, Converter};
    use crate::{
        command::LowCommand,
        controller::high_converter::playlist_set::tests::playlist_items_with_urls,
        vlc_responses::PlaylistItem,
    };

    macro_rules! assert_compare {
        (
            items = $items:expr;
            start = $start:expr;
            command_url = $command_url:expr;
            command_next_urls = $command_next_urls:expr;
            $expected:expr
        ) => {{
            let items: &[PlaylistItem] = $items;
            let items_debug: Vec<_> = items
                .iter()
                .map(|item| format!("{}=>{}", item.id, item.url.to_string()))
                .collect();
            let start: ComparisonStart = $start;
            let command_url: &str = $command_url;
            let current_or_past_url = file_url(command_url);
            let current_or_past_url_str = current_or_past_url.to_string();
            let command_next_urls: Vec<&str> = $command_next_urls;
            let next_urls: Vec<_> = command_next_urls.iter().copied().map(file_url).collect();
            let next_urls_debug: Vec<_> = next_urls.iter().map(url::Url::to_string).collect();
            let command = Command {
                current_or_past_url,
                next_urls,
                max_history_count: 1.try_into().expect("nonzero"),
            };
            assert_eq!(
                Converter::compare(&items, start, &command),
                $expected.into(),
                "items {items_debug:?},
                start {start:?},
                command.current_or_past_url {current_or_past_url_str:?},
                command.next_urls {next_urls_debug:?}"
            );
        }};
    }

    #[test]
    fn empty() {
        assert_compare!(
            items = &items![];
            start = ComparisonStart::at(0);
            command_url = "current";
            command_next_urls = vec![];
            Ok(())
        );
    }
    #[test]
    fn adds_current() {
        let full_items = items!["a", "b", "c", "d"];
        for start_idx in 0..full_items.len() {
            let items = &full_items[start_idx..];
            let url = file_url("current");
            assert_compare!(
                items = items;
                start = ComparisonStart::at(items.len()).include_current();
                command_url = "current";
                command_next_urls = vec![];
                Err(LowCommand::PlaylistAdd { url })
            );
        }
    }
    #[test]
    fn adds_next_url() {
        let full_item_strs = &["apple", "banana", "cucumber", "dates", "eggplant"];
        let items = playlist_items_with_urls(full_item_strs);
        for start_idx in 0..(items.len() - 1) {
            let items = &items[start_idx..];
            let url = file_url(full_item_strs[start_idx + 1]);
            assert_compare!(
                items = items;
                start = ComparisonStart::at(items.len());
                command_url = full_item_strs[start_idx];
                command_next_urls = full_item_strs[(start_idx+1)..].to_vec();
                Err(LowCommand::PlaylistAdd { url })
            );
        }
    }
    #[test]
    fn deletes_mismatch_current() {
        let full_items = items!["alfalfa", "beets", "bears", "beetles"];
        for start_idx in 0..full_items.len() {
            let items = &full_items[start_idx..];
            for compare_idx in 0..(items.len() - 1) {
                let item_id = (start_idx + compare_idx).to_string();
                assert_compare!(
                    items = items;
                    start = ComparisonStart::at(compare_idx).include_current();
                    command_url = "command";
                    command_next_urls = vec![];
                    Err(LowCommand::PlaylistDelete { item_id })
                );
            }
        }
    }
    #[test]
    fn deletes_mismatch_next_url() {
        let full_item_strs = &["alfalfa", "beets", "bears", "beetles"];
        let full_items = playlist_items_with_urls(full_item_strs);
        for start_idx in 0..full_items.len() {
            let items = &full_items[start_idx..];
            for compare_idx in 0..items.len() {
                dbg!((start_idx, compare_idx));
                let item_id = (start_idx + 1).max(start_idx + compare_idx);
                let mismatched_url = "something";
                let expected_action = if item_id < full_items.len() {
                    let item_id = item_id.to_string();
                    Err(LowCommand::PlaylistDelete { item_id })
                } else {
                    let url = file_url(mismatched_url);
                    Err(LowCommand::PlaylistAdd { url })
                };
                assert_compare!(
                    items = items;
                    start = ComparisonStart::at(compare_idx);
                    command_url = "irrelevant";
                    command_next_urls = vec![full_item_strs[start_idx], mismatched_url, "very", "different"];
                    expected_action
                );
                // remainder of list is OK
                assert_compare!(
                    items = items;
                    start = ComparisonStart::at(compare_idx).include_current();
                    command_url = full_item_strs[start_idx + compare_idx];
                    command_next_urls = full_item_strs[(start_idx + compare_idx + 1)..].to_vec();
                    Ok(())
                );
                assert_compare!(
                    items = items;
                    start = ComparisonStart::at(compare_idx);
                    command_url = "irrelevant";
                    command_next_urls = full_item_strs[(start_idx + compare_idx)..].to_vec();
                    Ok(())
                );
            }
        }
    }
}
