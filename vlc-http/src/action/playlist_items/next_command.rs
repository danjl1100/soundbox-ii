// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use super::insert_match::{find_insert_match, MatchAction};
use super::Target;
use crate::{response::playlist::Item, Command};

impl Target {
    pub(super) fn next_command(
        &self,
        playlist: &[Item],
        playing_item_index: Option<usize>,
    ) -> Option<Command> {
        // let (insert_match, playing_index) = if let Some(playing_item_index) = playing_item_index {
        //     todo!()
        //     // let playlist_urls: Vec<_> = playlist[..=playing_item_index]
        //     //     .iter()
        //     //     .map(|item| &item.url)
        //     //     .collect();
        //     // let insert_match = find_insert_match(&self.target.urls, &playlist_urls);
        //     // (insert_match, playlist_urls.len().checked_sub(1))
        // } else {
        //     let playlist_urls: Vec<_> = playlist.iter().map(|item| &item.url).collect();
        //     let insert_match = find_insert_match(&self.urls, &playlist_urls);
        //     (insert_match, None::<usize>)
        // };

        // TODO remove `trim_offset` if unused
        let (playlist_trimmed, _trim_offset) =
            playing_item_index.map_or((playlist, 0), |playing_item_index| {
                let offset = playing_item_index.saturating_sub(1);
                (&playlist[offset..], offset)
            });
        let playlist_urls_trimmed: Vec<_> = playlist_trimmed.iter().map(|item| &item.url).collect();
        let insert_match = find_insert_match(&self.urls, &playlist_urls_trimmed);

        // delete first entry to match `max_history_count`
        let items_before_match_start = insert_match
            .match_start
            // .or(playing_index.map(|index| {
            //     // len
            //     index + 1
            // }))
            .unwrap_or(playlist.len());
        let max_history_count = usize::from(self.max_history_count.get());

        // println!("playlist_urls = [");
        // for item in playlist {
        //     println!("    \"{}\",", item.url);
        // }
        // println!("]");

        // println!("target urls = [");
        // for url in &self.target.urls {
        //     println!("    \"{url}\",");
        // }
        // println!("]");

        // println!(
        //     "insert_match = {{ match_start: {:?}, next_to_insert: {:?} }}",
        //     insert_match.match_start,
        //     insert_match.next_to_insert.map(url::Url::as_str)
        // );

        // println!("playing_index = {playing_index:?}");

        // dbg!(items_before_match_start, max_history_count);
        // let matched_start = insert_match.match_start.unwrap_or(playlist.len());

        // Size of (1. History) + (2. Current playing item), crucially excluding (4. Matched)
        // let len_history = playing_index.map_or(match_start

        if items_before_match_start > max_history_count && !playlist.is_empty() {
            return Some(Command::PlaylistDelete {
                item_id: playlist[0].id,
            });
        }

        match insert_match.next {
            Some(MatchAction::InsertValue(next)) => {
                return Some(Command::PlaylistAdd { url: next.clone() });
            }
            Some(MatchAction::DeleteIndex(index)) => {
                return Some(Command::PlaylistDelete {
                    item_id: playlist[index].id,
                });
            }
            None => {}
        }

        None
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    const ID_OFFSET: u64 = 200;

    fn to_url(input: &&str) -> url::Url {
        url::Url::parse(&format!("file://{input}")).expect("valid URL test param")
    }
    fn strs_to_urls(input: &[&str]) -> Vec<url::Url> {
        input.iter().map(to_url).collect()
    }
    fn strs_to_items(input: &[&str]) -> Vec<Item> {
        input
            .iter()
            .map(to_url)
            .enumerate()
            .map(|(index, url)| Item {
                url,
                id: u64::try_from(index).expect("sane test param length") + ID_OFFSET,
                duration_secs: None,
                name: String::new(),
            })
            .collect()
    }

    #[allow(clippy::unnecessary_wraps)] // convenience for tests
    fn delete_nth(n: u64) -> Option<Command> {
        Some(Command::PlaylistDelete {
            item_id: n + ID_OFFSET,
        })
    }
    #[allow(clippy::unnecessary_wraps)] // convenience for tests
    fn add(url: &str) -> Option<Command> {
        Some(Command::PlaylistAdd { url: to_url(&url) })
    }

    fn target(target_urls: &[&str]) -> Uut {
        target_history(u16::MAX, target_urls)
    }
    fn target_history(max_history_count: u16, target_urls: &[&str]) -> Uut {
        Uut {
            target: Target {
                urls: strs_to_urls(target_urls),
                max_history_count: max_history_count.try_into().expect("nonzero test param"),
            },
        }
    }
    struct Uut {
        target: Target,
    }
    impl Uut {
        fn check(&self, existing: &[&str]) -> Option<Command> {
            self.check_playing(None, existing)
        }
        fn check_playing(
            &self,
            playing_item_index: Option<usize>,
            existing: &[&str],
        ) -> Option<Command> {
            let existing = strs_to_items(existing);
            self.target.next_command(&existing, playing_item_index)
        }
    }

    #[test]
    fn removes_history() {
        let uut = target_history(2, &["M1"]);
        assert_eq!(uut.check(&["X1"]), add("M1"));
        assert_eq!(uut.check(&["X1", "X2"]), add("M1"));
        assert_eq!(uut.check(&["X1", "X2", "X3"]), delete_nth(0));
        assert_eq!(uut.check(&["X1", "X2", "X3", "X4"]), delete_nth(0));
    }

    #[test]
    #[ignore = "TODO"] // TODO
    fn removes_trailing_items() {
        let uut = target(&["M1", "M2", "M3"]);
        assert_eq!(uut.check(&["M1"]), add("M2"));
        assert_eq!(uut.check(&["M1", "M2"]), add("M3"));
        assert_eq!(uut.check(&["M1", "M2", "X1"]), delete_nth(2));
    }
}
