// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use super::{Command, Converter, LowCommand, PlaylistItem};

impl Converter {
    pub(super) fn enqueue_items(
        comparison_items: &[PlaylistItem],
        command: &Command,
    ) -> Result<Option<String>, LowCommand> {
        let mut comparison_items = comparison_items.iter();
        let mut first_matched_id = None;
        'outer: for url in &command.urls {
            let url_str = url.to_string();
            for existing in comparison_items.by_ref() {
                if existing.url == url_str {
                    // matched existing, continue to next command_url
                    if first_matched_id.is_none() {
                        first_matched_id = Some(existing.id.clone());
                    }
                    continue 'outer;
                }
            }
            // exhausted existing urls, need to Add
            return Err(LowCommand::PlaylistAdd { url: url.clone() });
        }
        // all command urls paired to existing urls (in-order)
        Ok(first_matched_id)
    }
    pub(super) fn delete_items(
        comparison_items: &[PlaylistItem],
        command: &Command,
    ) -> Result<(), LowCommand> {
        let mut command_url_strs = command.urls.iter().map(url::Url::to_string);
        for existing in comparison_items {
            match command_url_strs.next() {
                Some(url_str) if url_str == existing.url => {}
                _ => {
                    let item_id = existing.id.clone();
                    return Err(LowCommand::PlaylistDelete { item_id });
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::file_url;
    use super::*;

    macro_rules! items_cmd {
        (
            [ $($item:expr),* ] => [ $($url:expr),* ]
        ) => {{
            let items: &[&str] = &[ $($item),* ];
            let items: Vec<PlaylistItem> = items![; ..items];
            let urls: &[&str] = &[ $($url),* ];
            let urls = urls.iter().copied().map(file_url).collect();
            let cmd = Command {
                urls,
                max_history_count: 1.try_into().expect("nonzero"),
            };
            (items, cmd)
        }};
    }
    macro_rules! assert_enqueue {
        (
            [ $($item:expr),* ] => [ $($url:expr),* ];
            $expected:expr
        ) => {{
            let (items, cmd) = items_cmd!([$($item),*] => [$($url),*]);
            assert_eq!(Converter::enqueue_items(&items, &cmd), $expected);
        }};
    }
    macro_rules! assert_delete {
        (
            [ $($item:expr),* ] => [ $($url:expr),* ];
            $expected:expr
        ) => {{
            let (items, cmd) = items_cmd!([$($item),*] => [$($url),*]);
            assert_eq!(Converter::delete_items(&items, &cmd), $expected);
        }};
    }

    fn add<T>(url_str: &str) -> Result<T, LowCommand> {
        Err(LowCommand::PlaylistAdd {
            url: file_url(url_str),
        })
    }

    #[test]
    fn empty() {
        assert_enqueue!(
            [] => [];
            Ok(None)
        );
        assert_delete!(
            [] => [];
            Ok(())
        );
    }
    #[test]
    fn adds_next() {
        assert_enqueue!([] => ["a"]; add("a"));
        assert_enqueue!(["b"] => ["a"]; add("a"));
        assert_enqueue!(["a", "b"] => ["a"]; Ok(Some(0.to_string())));
        assert_enqueue!(["a", "b", "c"] => ["a", "c", "d"]; add("d"));
        assert_enqueue!(["d", "c", "b", "a"] => ["a", "b"]; add("b"));
        assert_enqueue!(["d", "c", "b", "a"] => ["b", "c"]; add("c"));
        assert_enqueue!(["d", "c", "b", "a"] => ["c", "d"]; add("d"));
        assert_enqueue!(["d", "c", "b", "a"] => ["d", "c", "b", "a"]; Ok(Some(0.to_string())));
        assert_enqueue!(["d", "c", "b", "a"] => ["a", "c", "b", "a"]; add("c"));
    }

    fn delete(id: usize) -> Result<(), LowCommand> {
        Err(LowCommand::PlaylistDelete {
            item_id: id.to_string(),
        })
    }
    #[test]
    fn deletes_first() {
        assert_delete!(["a"] => []; delete(0));
        assert_delete!(["a"] => ["a"]; Ok(()));
        assert_delete!(["a", "b", "c"] => ["a", "b"]; delete(2));
        assert_delete!(["a", "b", "c"] => ["a", "c"]; delete(1));
        assert_delete!(["a", "b", "c"] => ["b", "c"]; delete(0));
        assert_delete!(["a", "b", "c"] => ["a"]; delete(1));
        assert_delete!(["a", "c"] => ["a"]; delete(1));
    }
    #[test]
    fn deletes_gap_in_cmd() {
        assert_delete!(["a", "b", "c", "b", "e", "f"] => ["a", "c", "b", "e", "f"]; delete(1));
        assert_delete!(["a", "b", "a", "b", "a", "b"] => ["a", "b", "a", "a", "b"]; delete(3));
        assert_delete!(["a", "b", "a", "a", "b"] => ["a", "b", "a", "a", "b"]; Ok(()));
    }
}
