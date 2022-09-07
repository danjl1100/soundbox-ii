// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use super::{Command, Converter};
use crate::controller::LowCommand;
use crate::vlc_responses::PlaylistItem;

impl Converter {
    /// Emits a `PlaylistDelete` error for the first item in `items`
    /// if the current item has more than `command.max_history_count` preceeding items.
    ///
    /// For no current item, assumes current item is past the end of the list.
    pub(super) fn remove_previous_items(
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
                Some(current_index) if current_index > (*max_history_count).into() => true,
                None if items.len() > (*max_history_count).into() => true,
                _ => false, // history length within bounds
            };
            if remove_first {
                let item_id = first_id.clone();
                Err(LowCommand::PlaylistDelete { item_id })?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        tests::{calc_current_item_index, file_url},
        Command, Converter,
    };
    use crate::{command::LowCommand, vlc_responses::PlaylistItem};

    macro_rules! assert_remove {
        (
            items = $items:expr;
            current_url = $current_url:expr;
            max_history_count = $max_history_count:expr;
            $expected:expr
        ) => {
            {
                let items: &[PlaylistItem] = $items;
                let items_debug: Vec<_> = items.iter().map(|item| format!("{}=>{}", item.id, item.url.to_string())).collect();
                let current_url: Option<&str> = $current_url;
                let current_url: Option<String> = current_url.map(|s| file_url(s).to_string());
                let current_index_item = calc_current_item_index(items, &current_url);
                let max_history_count: usize = $max_history_count;
                let command = command_with_max_history_count(max_history_count);
                assert_eq!(
                    Converter::remove_previous_items(items, current_index_item, &command),
                    $expected.into(),
                    "items {items_debug:?}, current_url {current_url:?}, max_history_count {max_history_count}"
                );
            }
        };
    }
    fn command_with_max_history_count(max_history_count: usize) -> Command {
        Command {
            current_or_past_url: url::Url::parse("file:///").expect("url"),
            next_urls: vec![],
            max_history_count: max_history_count.try_into().expect("nonzero"),
        }
    }

    #[test]
    fn ignores_empty() {
        assert_remove!(
            items = &items![];
            current_url = None;
            max_history_count = 1;
            Ok(())
        );
        assert_remove!(
            items = &items![];
            current_url = Some("a");
            max_history_count = 1;
            Ok(())
        );
    }
    #[test]
    fn one() {
        for item_num in 0..100 {
            let item_id = item_num.to_string();
            assert_remove!(
                items = &items![item_num => "alpha", 200 => "beta"];
                current_url = None;
                max_history_count = 1;
                Err(LowCommand::PlaylistDelete { item_id })
            );
        }
    }
    #[test]
    fn keeps_current() {
        // NOTE: Arguably, since `max_history_count` is NonZeroUsize, this test is pointless
        // but it does verify the behavior is identical to before ...?
        for item_num in 0..100 {
            assert_remove!(
                items = &items![item_num => "alpha"];
                current_url = Some("alpha");
                max_history_count = 1;
                Ok(())
            );
        }
    }
    #[test]
    fn keeps_before_current() {
        for max_history_count in 1..20 {
            for first_id in 0..10 {
                let expected = if max_history_count >= 3 {
                    Ok(())
                } else {
                    let item_id = first_id.to_string();
                    Err(LowCommand::PlaylistDelete { item_id })
                };
                assert_remove!(
                    items = &items![first_id=>"go", 10=>"dog", 20=>"spot", 31=>"runs"];
                    current_url = Some("runs");
                    max_history_count = max_history_count;
                    expected
                );
            }
        }
    }
    #[test]
    fn keeps_no_current() {
        for max_history_count in 1..20 {
            for first_id in 0..10 {
                let expected = if max_history_count >= 4 {
                    Ok(())
                } else {
                    let item_id = first_id.to_string();
                    Err(LowCommand::PlaylistDelete { item_id })
                };
                assert_remove!(
                    items = &items![first_id=>"go", 10=>"dog", 20=>"spot", 31=>"runs"];
                    current_url = None;
                    max_history_count = max_history_count;
                    expected
                );
            }
        }
    }
}
