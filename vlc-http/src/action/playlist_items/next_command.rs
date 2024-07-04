// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use super::insert_match::{find_insert_match, MatchAction};
use super::Target;

impl<T> Target<T> {
    pub(super) fn next_command<'a, 'b, U>(
        &'a self,
        playlist: &'b [U],
        playing_item_index: Option<usize>,
    ) -> Option<NextCommand<'a, T, U>>
    where
        U: AsRef<T>,
        T: std::cmp::Eq + std::fmt::Debug,
        'b: 'a,
    {
        let trim_offset = playing_item_index.unwrap_or(0);
        let insert_match = find_insert_match(&self.urls, &playlist[trim_offset..]);

        // delete first entry to match `max_history_count`
        let trimmed_items_before_match_start = insert_match
            .match_start
            .unwrap_or(playlist.len() - trim_offset);

        let delete_first_item = {
            let undesired_items_count = playing_item_index.unwrap_or(
                // none playing, count before match_start (adjust to global)
                trimmed_items_before_match_start + trim_offset,
            );
            let max_history_count = usize::from(self.max_history_count);

            (undesired_items_count > max_history_count).then(|| {
                playlist
                    .first()
                    .expect("playlist nonempty, items before playing/match")
            })
        };
        let delete_after_playing_item = {
            let item_after_playing =
                playing_item_index.and_then(|playing| playlist.get(playing + 1));
            item_after_playing
                .and_then(|item| (trimmed_items_before_match_start > 1).then_some(item))
        };

        let (insert_end, delete_end) = match insert_match.next {
            Some(MatchAction::InsertValue(url)) => (Some(url), None),
            Some(MatchAction::DeleteValue(value)) => (None, Some(value)),
            None => (None, None),
        };

        // precedence ordering A-D:

        // A. [#5] clear items from the end
        delete_end
            .map(NextCommand::PlaylistDelete)
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
            )
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
        fn check(&self, existing: &'static [TestItem]) -> Option<Cmd<'_>> {
            self.check_playing(None, existing)
        }
        fn check_playing(
            &self,
            playing_item_index: Option<usize>,
            existing: &'static [TestItem],
        ) -> Option<Cmd<'_>> {
            self.target.next_command(existing, playing_item_index)
        }
    }

    macro_rules! check {
        ($uut:expr => &[$($s:expr),* $(,)?]) => {
            {
                let items: &'static [TestItem] = &[$( TestItem($s) ),*];
                let uut: &Uut = $uut;
                uut.check(&items)
            }
        };
        ($uut:expr => Some($index:expr), &[$($s:expr),* $(,)?]) => {
            {
                let items: &'static [TestItem] = &[$( TestItem($s) ),*];
                let uut: &Uut = $uut;
                uut.check_playing(Some($index), &items)
            }
        };
        ($uut:expr => &[$($s:expr),* $(,)?], None) => {
            assert_eq!(check!($uut => &[$($s),*]), None);
        };
        ($uut:expr => Some($index:expr), &[$($s:expr),* $(,)?], None) => {
            assert_eq!(check!($uut => Some($index), &[$($s),*]), None);
        };
        ($uut:expr => &[$($s:expr),* $(,)?], add($url:expr)) => {
            let url: &'static str = $url;
            let expected = Some(Cmd::PlaylistAdd(&&url));
            assert_eq!(check!($uut => &[$($s),*]), expected);
        };
        ($uut:expr => Some($index:expr), &[$($s:expr),* $(,)?], add($url:expr)) => {
            let url: &'static str = $url;
            let expected = Some(Cmd::PlaylistAdd(&&url));
            assert_eq!(check!($uut => Some($index), &[$($s),*]), expected);
        };
        ($uut:expr => &[$($s:expr),* $(,)?], delete($item:expr)) => {
            let item: &'static str = $item;
            let item = &TestItem(item);
            let expected = Some(Cmd::PlaylistDelete(item));
            assert_eq!(check!($uut => &[$($s),*]), expected);
        };
        ($uut:expr => Some($index:expr), &[$($s:expr),* $(,)?], delete($item:expr)) => {
            let item: &'static str = $item;
            let item = &TestItem(item);
            let expected = Some(Cmd::PlaylistDelete(item));
            assert_eq!(check!($uut => Some($index), &[$($s),*]), expected);
        };
    }

    #[test]
    fn removes_history() {
        let uut = target_history(2, &["M1"]);
        check!(&uut => &["X1"], add("M1"));
        check!(&uut => &["X1", "X2"], add("M1"));
        check!(&uut => &["X1", "X2", "X3"], delete("X1"));
        check!(&uut => &["X1", "X2", "X3", "X4"], delete("X1"));
    }

    #[test]
    fn removes_trailing_items() {
        let uut = target(&["M1", "M2", "M3"]);
        check!(&uut => &["M1"], add("M2"));
        check!(&uut => &["M1", "M2"], add("M3"));
        check!(&uut => &["M1", "M2", "X1"], delete("X1"));
        check!(&uut => &["M1", "M2", "M3"], None);

        // "trailing" (X1) is higher precedence than "prior" (X0)
        check!(&uut => &["X0", "M1", "M2", "M3", "X1"], delete("X1"));

        // ---

        // when *NOTHING* is playing,
        // first "trailing" (X1) is highest precedence
        check!(&uut => &["_", "X0", "M1", "X1", "M2", "M3", "X2"], delete("X1"));
        check!(&uut => &["_", "X0", "M1", "M2", "X1", "M3", "X2"], delete("X1"));

        // when playing *IS* desired,
        // first "trailing" (X1) is higher precedence than "leading" (X0)
        check!(&uut => Some(2), &["_", "X0", "M1", "X1", "M2", "M3", "X2"], delete("X1"));
        check!(&uut => Some(2), &["_", "X0", "M1", "M2", "X1", "M3", "X2"], delete("X1"));

        // when playing is *NOT* desired,
        // first "trailing" (X1) is higher precedence than "leading" (X0)
        check!(&uut => Some(0), &["_", "X0", "M1", "X1", "M2", "M3", "X2"], delete("X1"));
        check!(&uut => Some(0), &["_", "X0", "M1", "M2", "X1", "M3", "X2"], delete("X1"));

        // finish out the scenario above
        check!(&uut => Some(0), &["_", "X0", "M1", "M2", "M3", "X2"], delete("X2"));
        check!(&uut => Some(0), &["_", "X0", "M1", "M2", "M3"], delete("X0"));
        check!(&uut => Some(0), &["_", "M1", "M2", "M3"], None);
    }

    #[test]
    fn removes_between_playing_and_match() {
        let uut = target(&["M1", "M2", "M3"]);
        check!(&uut => Some(0), &["_", "X0", "X1", "X2", "M1", "M2", "M3"], delete("X0"));
        check!(&uut => Some(0), &["_", "X1", "X2", "M1", "M2", "M3"], delete("X1"));
        check!(&uut => Some(0), &["_", "X2", "M1", "M2", "M3"], delete("X2"));
        check!(&uut => Some(0), &["_", "M1", "M2", "M3"], None);
    }

    #[test]
    fn persists_history_anticipating_next() {
        let uut = target_history(3, &["M1", "M2", "M3"]);
        //                         \/
        check!(&uut => Some(0), &["X0", "X1", "X2", "X3"], add("M1"));
        check!(&uut => Some(0), &["X0", "X1", "X2", "X3", "M1", "M2", "M3"], delete("X1"));
        //                         1     \/
        check!(&uut => Some(1), &["X0", "X1", "X2", "X3"], add("M1"));
        check!(&uut => Some(1), &["X0", "X1", "X2", "X3", "M1", "M2", "M3"], delete("X2"));
        //                         1     2     \/
        check!(&uut => Some(2), &["X0", "X1", "X2", "X3"], add("M1"));
        check!(&uut => Some(2), &["X0", "X1", "X2", "X3", "M1", "M2", "M3"], delete("X3"));
        //                         1     2     3     \/
        check!(&uut => Some(3), &["X0", "X1", "X2", "X3"], add("M1"));
        check!(&uut => Some(3), &["X0", "X1", "X2", "X3", "M1", "M2", "M3"], None);
        //                         X     1     2     3     \/
        check!(&uut => Some(4), &["X0", "X1", "X2", "X3", "X4"], delete("X0"));
        check!(&uut => Some(4), &["X0", "X1", "X2", "X3", "X4", "M1", "M2", "M3"], delete("X0"));

        check!(&uut => Some(2), &["X1", "X2", "P"], add("M1"));
    }

    #[test]
    fn match_before_playing_starts_again() {
        let uut = target(&["M1", "M2", "M3"]);
        //                                          3\/
        check!(&uut => Some(3), &["M1", "M2", "M3", "P"], add("M1"));
        check!(&uut => Some(3), &["M1", "M2", "M3", "P", "M1"], add("M2"));
        check!(&uut => Some(3), &["M1", "M2", "M3", "P", "M1", "M2"], add("M3"));
        check!(&uut => Some(3), &["M1", "M2", "M3", "P", "M1", "M2", "M3"], None);
    }

    #[test]
    fn history_trims_before_match_only() {
        let uut = target_history(3, &["M1", "M2", "M3", "M4", "M5"]);
        //
        check!(&uut => &[], add("M1"));
        check!(&uut => &["M1"], add("M2"));
        check!(&uut => &["M1","M2"], add("M3"));
        check!(&uut => &["M1","M2","M3"], add("M4"));
        check!(&uut => &["M1","M2","M3","M4"], add("M5"));
        check!(&uut => &["M1","M2","M3","M4","M5"], None);
        //
        check!(&uut => Some(0), &["_"], add("M1"));
        check!(&uut => Some(0), &["_","M1"], add("M2"));
        check!(&uut => Some(0), &["_","M1","M2"], add("M3"));
        check!(&uut => Some(0), &["_","M1","M2","M3"], add("M4"));
        check!(&uut => Some(0), &["_","M1","M2","M3","M4"], add("M5"));
        check!(&uut => Some(0), &["_","M1","M2","M3","M4","M5"], None);
        //
        check!(&uut => &["_"], add("M1"));
        check!(&uut => &["_","M1"], add("M2"));
        check!(&uut => &["_","M1","M2"], add("M3"));
        check!(&uut => &["_","M1","M2","M3"], add("M4"));
        check!(&uut => &["_","M1","M2","M3","M4"], add("M5"));
        check!(&uut => &["_","M1","M2","M3","M4","M5"], None);
    }
}
