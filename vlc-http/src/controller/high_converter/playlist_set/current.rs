// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use super::{Command, ComparisonStart, Converter};
use crate::controller::LowCommand;
use crate::vlc_responses::PlaylistItem;

impl Converter {
    /// Locates the comparison start point at/after the commanded item
    pub(super) fn prep_comparison_start(
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
}

#[cfg(test)]
mod tests {
    use super::super::{
        tests::{calc_current_item_index, file_url},
        Command, ComparisonStart, Converter,
    };
    use crate::{
        command::LowCommand,
        controller::high_converter::playlist_set::tests::playlist_items_with_urls,
        vlc_responses::PlaylistItem,
    };

    macro_rules! assert_start {
        (
            items = $items:expr;
            current_url = $current_url:expr;
            command_url = $command_url:expr;
            $expected:expr
        ) => {{
            let mut converter = Converter::new();
            assert_start!(items=$items; current_url=$current_url; command_url=$command_url;
                          converter = &mut converter;
                          $expected);
        }};
        (
            items = $items:expr;
            current_url = $current_url:expr;
            command_url = $command_url:expr;
            converter = $converter:expr;
            $expected:expr
        ) => {{
            let items: &[PlaylistItem] = $items;
            let items_debug: Vec<_> = items
                .iter()
                .map(|item| format!("{}=>{}", item.id, item.url.to_string()))
                .collect();
            let current_url: Option<&str> = $current_url;
            let current_url: Option<String> = current_url.map(|s| file_url(s).to_string());
            let current_index_item = calc_current_item_index(items, &current_url);
            let command_url: &str = $command_url;
            let current_or_past_url = file_url(command_url);
            let current_or_past_url_str = current_or_past_url.to_string();
            let command = Command {
                current_or_past_url,
                next_urls: vec![],
                max_history_count: 1.try_into().expect("nonzero"),
            };
            let converter: &mut Converter = $converter;
            let converter_clone = (*converter).clone();
            assert_eq!(
                converter.prep_comparison_start(&items, current_index_item, &command),
                $expected.into(),
                "items {items_debug:?},
                current_url {current_url:?},
                command.current_or_past_url {current_or_past_url_str:?},
                converter {converter_clone:?}"
            );
        }};
    }

    #[test]
    fn current_not_active_last_starts_it_once() {
        for item_num in 10..100 {
            let item_id = Some(item_num.to_string());
            let mut converter = Converter::new();
            let items = items![9=>"baa", 909=>"crikey", item_num=>"needle"];
            let command_url = "needle";
            assert_start!(
                items = &items;
                current_url = None;
                command_url = command_url;
                converter = &mut converter;
                Err(LowCommand::PlaylistPlay { item_id })
            );
            // second time, no PlaylistPlay action left
            assert_start!(
                items = &items;
                current_url = None;
                command_url = command_url;
                converter = &mut converter;
                Ok(ComparisonStart::at(items.len()))
            );
        }
    }
    #[test]
    fn empty_starts_at_first() {
        assert_start!(
            items = &items![];
            current_url = None;
            command_url = "a";
            Ok(ComparisonStart::at(0).include_current())
        );
    }
    #[test]
    fn current_starts_after_first() {
        assert_start!(
            items = &items!["a"];
            current_url = Some("a");
            command_url = "a";
            Ok(ComparisonStart::at(1))
        );
    }
    #[test]
    fn locates_after_current() {
        const UNINHABITED: &str = "uninhabited";
        let full_item_strs = &[
            "a",
            "b",
            "c",
            "d",
            "danzon",
            "triste",
            "pobre tomato",
            "salsa",
            "caramel",
        ];
        let full_items = playlist_items_with_urls(full_item_strs);
        for start_idx in (0..(full_items.len() - 1)).rev() {
            let items = &full_items[start_idx..];
            // let full_item_strs = &full_item_strs[start_idx..];
            for (commanded_idx, (commanded_id_num, commanded)) in full_item_strs
                .iter()
                .copied()
                .enumerate()
                .skip(start_idx)
                .enumerate()
            {
                let commanded_is_last = (commanded_idx + 1) == items.len();
                for current in full_item_strs
                    .iter()
                    .skip(start_idx)
                    .copied()
                    .chain(std::iter::once(UNINHABITED))
                    .map(Option::Some)
                    .chain(std::iter::once(None))
                {
                    let current_id_num = current.and_then(|current| {
                        full_item_strs
                            .iter()
                            .enumerate()
                            .find(|(_, s)| *s == &current)
                            .map(|(idx, _)| idx)
                    });
                    let previous = current_id_num
                        .and_then(|c| c.checked_sub(1))
                        .and_then(|previous_idx| full_item_strs.get(previous_idx).copied());
                    let commanded_is_previous = previous.map_or(false, |p| p == commanded);
                    let current_filtered = current.filter(|c| *c != UNINHABITED);
                    let expected = match current_filtered {
                        Some(current) if commanded == current || commanded_is_previous => {
                            // `command.current_or_past_url` is satisfied.  continue after that index
                            if commanded_is_last {
                                Ok(ComparisonStart::at(items.len())) // compare END
                            } else {
                                Ok(ComparisonStart::at(commanded_idx + 1)) // compare AFTER COMMANDED
                            }
                        }
                        Some(_) => {
                            // unsatisfied `command.current_or_past_url`,
                            // DELETE the incorrect `current` (rewrites fraction-of-a-track history)
                            let item_id = current_id_num
                                .expect("inhabited current has id num/index")
                                .to_string();
                            Err(LowCommand::PlaylistDelete { item_id })
                        }
                        None => {
                            // No current track, executing on END only
                            if commanded_is_last {
                                // PLAY, commanded is last
                                let item_id = Some(commanded_id_num.to_string());
                                Err(LowCommand::PlaylistPlay { item_id })
                            } else {
                                // compare END, incl current
                                Ok(ComparisonStart::at(items.len()).include_current())
                            }
                        }
                    };
                    assert_start!(
                        items = items;
                        current_url = current;
                        command_url = commanded;
                        expected
                    );
                }
            }
        }
    }
    #[test]
    fn locates_including_current() {
        let full_items = items!["the", "at", "of", "when", "why", "how"];
        for start_idx in 0..full_items.len() {
            let items = &full_items[start_idx..];
            let expected = Ok(ComparisonStart::at(items.len()).include_current());
            assert_start!(
                items = items;
                current_url = Some("parallelogram");
                command_url = "some command";
                expected
            );
        }
    }
}
