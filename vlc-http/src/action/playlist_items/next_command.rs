// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use super::insert_match::{find_insert_match, MatchAction};
use super::Target;

impl<T> Target<T> {
    /// Returns the next command and the matching subset of non-playing target items
    pub(super) fn next_command<'a, 'b, U>(
        &'a self,
        playlist: &'b [U],
        playing_item_index: Option<usize>,
    ) -> (Option<NextCommand<'a, T, U>>, &'b [U])
    where
        U: AsRef<T>,
        T: std::cmp::Eq + std::fmt::Debug,
        'b: 'a,
    {
        let trim_offset = if let Some(playing) = playing_item_index {
            assert!(
                playing < playlist.len(),
                "playing_item_index out of bounds of playlist"
            );
            playing
        } else {
            0
        };
        let insert_match = find_insert_match(&self.urls, &playlist[trim_offset..]);

        let items_before_match_start = insert_match.match_start.map_or(
            ItemsBeforeMatchStart::Absolute(playlist.len()),
            ItemsBeforeMatchStart::Trimmed,
        );

        // delete first entry to match `max_history_count`
        let delete_first_item = {
            let undesired_items_count = match (playing_item_index, items_before_match_start) {
                // count before playing
                (Some(playing_item_index), _) => playing_item_index,
                // none playing, count before match_start (adjusted to global)
                (
                    None,
                    // NOTE: Trimmed == Absolute when playing_item_index is None (e.g. trim_offset == 0)
                    ItemsBeforeMatchStart::Trimmed(absolute)
                    | ItemsBeforeMatchStart::Absolute(absolute),
                ) => absolute,
            };
            let max_history_count = usize::from(self.max_history_count);

            (undesired_items_count > max_history_count).then(|| {
                playlist
                    .first()
                    .expect("playlist nonempty, items before playing/match")
            })
        };
        // delete after the playing item, before the matched item
        let delete_after_playing_item = {
            playing_item_index.and_then(|playing| {
                let trimmed_items_before_match_start = match items_before_match_start {
                    ItemsBeforeMatchStart::Trimmed(trimmed) => trimmed,
                    ItemsBeforeMatchStart::Absolute(absolute) => absolute - playing,
                };
                let item_after_playing = playlist.get(playing + 1)?;
                (trimmed_items_before_match_start > 1).then_some(item_after_playing)
            })
        };

        let (insert_end, delete_end) = match insert_match.next {
            Some(MatchAction::InsertValue(url)) => (Some(url), None),
            Some(MatchAction::DeleteValue(value)) => (None, Some(value)),
            None => (None, None),
        };

        // precedence ordering A-D:

        let next_command = (
            // A. [#5] clear items from the end
            delete_end.map(NextCommand::PlaylistDelete)
        )
        .or(
            // B. [#1] clear history items from beginning
            delete_first_item.map(NextCommand::PlaylistDelete),
        )
        .or(
            // C. [#4] Add new item to end
            insert_end.map(NextCommand::PlaylistAdd),
        )
        .or(
            // D. [#3] clear items between playing and first desired
            delete_after_playing_item.map(NextCommand::PlaylistDelete),
        );

        let matched_after_playing = {
            // `insert_match` starts at/after playing item
            let matched_subset = insert_match.matched_subset;
            // remove first element if playing >= matched offset
            match playing_item_index
                .zip(insert_match.match_start)
                .zip(matched_subset.split_first().map(|(_first, rest)| rest))
            {
                Some(((playing, matched), rest)) if playing >= (matched + trim_offset) => rest,
                _ => matched_subset,
            }
        };

        (next_command, matched_after_playing)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum NextCommand<'a, T, U> {
    PlaylistAdd(
        /// URL
        &'a T,
    ),
    PlaylistDelete(
        /// Item
        &'a U,
    ),
}

#[derive(Clone, Copy, Debug)]
enum ItemsBeforeMatchStart {
    Trimmed(usize),
    Absolute(usize),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, PartialEq, Eq)]
    struct TestItem(&'static str);
    impl AsRef<&'static str> for TestItem {
        fn as_ref(&self) -> &&'static str {
            &self.0
        }
    }
    impl std::fmt::Debug for TestItem {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            <str as std::fmt::Debug>::fmt(self.0, f)
        }
    }

    type Cmd<'a> = NextCommand<'a, &'static str, TestItem>;

    fn target(target_urls: &[&'static str]) -> Uut {
        target_history(u16::MAX, target_urls)
    }
    fn target_history(max_history_count: u16, target_urls: &[&'static str]) -> Uut {
        Uut {
            target: Target {
                urls: target_urls.to_owned(),
                max_history_count,
            },
        }
    }
    struct Uut {
        target: Target<&'static str>,
    }
    impl Uut {
        fn check(&self, existing: &'static [TestItem]) -> (Option<Cmd<'_>>, &'static [TestItem]) {
            self.check_playing(None, existing)
        }
        fn check_playing(
            &self,
            playing_item_index: Option<usize>,
            existing: &'static [TestItem],
        ) -> (Option<Cmd<'_>>, &'static [TestItem]) {
            self.target.next_command(existing, playing_item_index)
        }
    }

    macro_rules! test_items {
        ($($item:expr),* $(,)?) => {{
           let items: &[TestItem] = &[ $(TestItem($item)),* ];
           items
        }};
    }

    macro_rules! check {
        // VALUES
        (@v $uut:expr => &[$($s:expr),* $(,)?]) => {
            {
                let items: &'static [TestItem] = &[$( TestItem($s) ),*];
                let uut: &Uut = $uut;
                uut.check(&items)
            }
        };
        (@v $uut:expr => Some($index:expr), &[$($s:expr),* $(,)?]) => {
            {
                let items: &'static [TestItem] = &[$( TestItem($s) ),*];
                let uut: &Uut = $uut;
                uut.check_playing(Some($index), &items)
            }
        };
        // ASSERTIONS
        ($uut:expr => &[$($s:expr),* $(,)?], (None, $matched:expr)) => {
            assert_eq!(check!(@v $uut => &[$($s),*]), (None, $matched));
        };
        ($uut:expr => Some($index:expr), &[$($s:expr),* $(,)?], (None, $matched:expr)) => {
            assert_eq!(check!(@v $uut => Some($index), &[$($s),*]), (None, $matched));
        };
        ($uut:expr => &[$($s:expr),* $(,)?], add($url:expr, $matched:expr)) => {
            let url: &'static str = $url;
            let expected = Some(Cmd::PlaylistAdd(&&url));
            assert_eq!(check!(@v $uut => &[$($s),*]), (expected, $matched));
        };
        ($uut:expr => Some($index:expr), &[$($s:expr),* $(,)?], add($url:expr, $matched:expr)) => {
            let url: &'static str = $url;
            let expected = Some(Cmd::PlaylistAdd(&&url));
            assert_eq!(check!(@v $uut => Some($index), &[$($s),*]), (expected, $matched));
        };
        ($uut:expr => &[$($s:expr),* $(,)?], delete($item:expr, $matched:expr)) => {
            let item: &'static str = $item;
            let item = &TestItem(item);
            let expected = Some(Cmd::PlaylistDelete(item));
            assert_eq!(check!(@v $uut => &[$($s),*]), (expected, $matched));
        };
        ($uut:expr => Some($index:expr), &[$($s:expr),* $(,)?], delete($item:expr, $matched:expr)) => {
            let item: &'static str = $item;
            let item = &TestItem(item);
            let expected = Some(Cmd::PlaylistDelete(item));
            assert_eq!(check!(@v $uut => Some($index), &[$($s),*]), (expected, $matched));
        };
    }

    const MATCH_EMPTY: &[TestItem] = &[];
    const MATCH1: &[TestItem] = test_items!["M1"];
    const MATCH2: &[TestItem] = test_items!["M2"];
    const MATCH3: &[TestItem] = test_items!["M3"];
    const MATCH12: &[TestItem] = test_items!["M1", "M2"];
    const MATCH23: &[TestItem] = test_items!["M2", "M3"];
    const MATCH123: &[TestItem] = test_items!["M1", "M2", "M3"];
    const MATCH1234: &[TestItem] = test_items!["M1", "M2", "M3", "M4"];
    const MATCH12345: &[TestItem] = test_items!["M1", "M2", "M3", "M4", "M5"];

    #[test]
    fn removes_history() {
        let uut = target_history(2, &["M1"]);
        check!(&uut => &["X1"], add("M1", MATCH_EMPTY));
        check!(&uut => &["X1", "X2"], add("M1", MATCH_EMPTY));
        check!(&uut => &["X1", "X2", "X3"], delete("X1", MATCH_EMPTY));
        check!(&uut => &["X1", "X2", "X3", "X4"], delete("X1", MATCH_EMPTY));
    }

    #[test]
    fn removes_trailing_items() {
        let uut = target(&["M1", "M2", "M3"]);
        check!(&uut => &["M1"], add("M2", MATCH1));
        check!(&uut => &["M1", "M2"], add("M3", MATCH12));
        check!(&uut => &["M1", "M2", "X1"], delete("X1", MATCH12));
        check!(&uut => &["M1", "M2", "M3"], (None, MATCH123));

        // "trailing" (X1) is higher precedence than "prior" (X0)
        check!(&uut => &["X0", "M1", "M2", "M3", "X1"], delete("X1", MATCH123));

        // ---

        // when *NOTHING* is playing,
        // first "trailing" (X1) is highest precedence
        check!(&uut => &["_", "X0", "M1", "X1", "M2", "M3", "X2"], delete("X1", MATCH1));
        check!(&uut => &["_", "X0", "M1", "M2", "X1", "M3", "X2"], delete("X1", MATCH12));

        // when playing *IS* desired,
        // first "trailing" (X1) is higher precedence than "leading" (X0)
        //                                   2\/
        check!(&uut => Some(2), &["_", "X0", "M1", "X1", "M2", "M3", "X2"], delete("X1", MATCH_EMPTY));
        check!(&uut => Some(2), &["_", "X0", "M1", "M2", "X1", "M3", "X2"], delete("X1", MATCH2));

        // when playing is *NOT* desired,
        // first "trailing" (X1) is higher precedence than "leading" (X0)
        check!(&uut => Some(0), &["_", "X0", "M1", "X1", "M2", "M3", "X2"], delete("X1", MATCH1));
        check!(&uut => Some(0), &["_", "X0", "M1", "M2", "X1", "M3", "X2"], delete("X1", MATCH12));

        // finish out the scenario above
        check!(&uut => Some(0), &["_", "X0", "M1", "M2", "M3", "X2"], delete("X2", MATCH123));
        check!(&uut => Some(0), &["_", "X0", "M1", "M2", "M3"], delete("X0", MATCH123));
        check!(&uut => Some(0), &["_", "M1", "M2", "M3"], (None, MATCH123));
    }

    #[test]
    fn removes_between_playing_and_match() {
        let uut = target(&["M1", "M2", "M3"]);
        check!(&uut => Some(0), &["_", "X0", "X1", "X2", "M1", "M2", "M3"], delete("X0", MATCH123));
        check!(&uut => Some(0), &["_", "X1", "X2", "M1", "M2", "M3"], delete("X1", MATCH123));
        check!(&uut => Some(0), &["_", "X2", "M1", "M2", "M3"], delete("X2", MATCH123));
        check!(&uut => Some(0), &["_", "M1", "M2", "M3"], (None, MATCH123));
    }

    #[test]
    fn persists_history_anticipating_next() {
        let uut = target_history(3, &["M1", "M2", "M3"]);
        //                         \/
        check!(&uut => Some(0), &["X0", "X1", "X2", "X3"], add("M1", MATCH_EMPTY));
        check!(&uut => Some(0), &["X0", "X1", "X2", "X3", "M1", "M2", "M3"], delete("X1", MATCH123));
        //                         1     \/
        check!(&uut => Some(1), &["X0", "X1", "X2", "X3"], add("M1", MATCH_EMPTY));
        check!(&uut => Some(1), &["X0", "X1", "X2", "X3", "M1", "M2", "M3"], delete("X2", MATCH123));
        //                         1     2     \/
        check!(&uut => Some(2), &["X0", "X1", "X2", "X3"], add("M1", MATCH_EMPTY));
        check!(&uut => Some(2), &["X0", "X1", "X2", "X3", "M1", "M2", "M3"], delete("X3", MATCH123));
        //                         1     2     3     \/
        check!(&uut => Some(3), &["X0", "X1", "X2", "X3"], add("M1", MATCH_EMPTY));
        check!(&uut => Some(3), &["X0", "X1", "X2", "X3", "M1", "M2", "M3"], (None, MATCH123));
        //                         X     1     2     3     \/
        check!(&uut => Some(4), &["X0", "X1", "X2", "X3", "X4"], delete("X0", MATCH_EMPTY));
        check!(&uut => Some(4), &["X0", "X1", "X2", "X3", "X4", "M1", "M2", "M3"], delete("X0", MATCH123));

        check!(&uut => Some(2), &["X1", "X2", "P"], add("M1", MATCH_EMPTY));
    }

    #[test]
    fn match_before_playing_starts_again() {
        let uut = target(&["M1", "M2", "M3"]);
        //                                          3\/
        check!(&uut => Some(3), &["M1", "M2", "M3", "P"], add("M1", MATCH_EMPTY));
        check!(&uut => Some(3), &["M1", "M2", "M3", "P", "M1"], add("M2", MATCH1));
        check!(&uut => Some(3), &["M1", "M2", "M3", "P", "M1", "M2"], add("M3", MATCH12));
        check!(&uut => Some(3), &["M1", "M2", "M3", "P", "M1", "M2", "M3"], (None, MATCH123));
    }

    #[test]
    fn history_trims_before_match_only() {
        let uut = target_history(3, &["M1", "M2", "M3", "M4", "M5"]);
        //
        check!(&uut => &[], add("M1", MATCH_EMPTY));
        check!(&uut => &["M1"], add("M2", MATCH1));
        check!(&uut => &["M1","M2"], add("M3", MATCH12));
        check!(&uut => &["M1","M2","M3"], add("M4", MATCH123));
        check!(&uut => &["M1","M2","M3","M4"], add("M5", MATCH1234));
        check!(&uut => &["M1","M2","M3","M4","M5"], (None, MATCH12345));
        //
        check!(&uut => Some(0), &["_"], add("M1", MATCH_EMPTY));
        check!(&uut => Some(0), &["_","M1"], add("M2", MATCH1));
        check!(&uut => Some(0), &["_","M1","M2"], add("M3", MATCH12));
        check!(&uut => Some(0), &["_","M1","M2","M3"], add("M4", MATCH123));
        check!(&uut => Some(0), &["_","M1","M2","M3","M4"], add("M5", MATCH1234));
        check!(&uut => Some(0), &["_","M1","M2","M3","M4","M5"], (None, MATCH12345));
        //
        check!(&uut => &["_"], add("M1", MATCH_EMPTY));
        check!(&uut => &["_","M1"], add("M2", MATCH1));
        check!(&uut => &["_","M1","M2"], add("M3", MATCH12));
        check!(&uut => &["_","M1","M2","M3"], add("M4", MATCH123));
        check!(&uut => &["_","M1","M2","M3","M4"], add("M5", MATCH1234));
        check!(&uut => &["_","M1","M2","M3","M4","M5"], (None, MATCH12345));
    }

    #[test]
    #[allow(clippy::similar_names)]
    fn matched_excludes_playing() {
        let uut123 = target(&["M1", "M2", "M3"]);
        let uut23 = target(&["M2", "M3"]);
        let uut3 = target(&["M3"]);
        //                           0\/
        check!(&uut123 => Some(0), &["_", "M1", "M2", "M3"], (None, MATCH123));
        //                                1\/
        check!(&uut123 => Some(1), &["_", "M1", "M2", "M3"], (None, MATCH23));
        //                                     2\/
        check!(&uut23 => Some(2), &["_", "M1", "M2", "M3"], (None, MATCH3));
        //                                          3\/
        check!(&uut3 => Some(3), &["_", "M1", "M2", "M3"], (None, MATCH_EMPTY));
    }

    #[test]
    fn deletes_first_when_no_match() {
        let uut = target_history(1, &[]);
        check!(&uut => Some(4), &["X0","X1","X2","X3","X4"], delete("X0", MATCH_EMPTY));
    }

    #[test]
    fn tolerates_all_empty() {
        let uut = target(&[]);
        check!(&uut => &[], (None, MATCH_EMPTY));
    }

    #[test]
    #[should_panic(expected = "playing_item_index out of bounds of playlist")]
    fn panics_for_playing_out_of_bounds() {
        let uut = target(&["M1"]);
        // S.I.C. ---------\2/     0     1
        check!(&uut => Some(2), &["X0", "X1"], (None, MATCH_EMPTY));
    }
}
