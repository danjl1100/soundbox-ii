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
        let playlist_trimmed = {
            let trim_offset = playing_item_index.unwrap_or(0).saturating_sub(1);
            &playlist[trim_offset..]
        };
        let insert_match = find_insert_match(&self.urls, playlist_trimmed);

        // delete first entry to match `max_history_count`
        let items_before_match_start = insert_match.match_start.unwrap_or(playlist.len());
        let max_history_count = usize::from(self.max_history_count.get());

        if items_before_match_start > max_history_count {
            return Some(NextCommand::PlaylistDelete {
                item: playlist
                    .first()
                    .expect("excess items implies there is at least one"),
            });
        }

        match insert_match.next {
            Some(MatchAction::InsertValue(url)) => Some(NextCommand::PlaylistAdd { url }),
            Some(MatchAction::DeleteIndex(index)) => Some(NextCommand::PlaylistDelete {
                item: &playlist_trimmed[index],
            }),
            None => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum NextCommand<'a, T, U> {
    PlaylistAdd { url: &'a T },
    PlaylistDelete { item: &'a U },
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
                max_history_count: max_history_count.try_into().expect("nonzero test param"),
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
        ($uut:expr => &[$($s:expr),* $(,)?], add($item:expr)) => {
            let item: &'static str = $item;
            let expected = Some(Cmd::PlaylistAdd { url: &&item });
            assert_eq!(check!($uut => &[$($s),*]), expected);
        };
        ($uut:expr => &[$($s:expr),* $(,)?], delete($item:expr)) => {
            let item: &'static str = $item;
            let item = &TestItem(item);
            let expected = Some(Cmd::PlaylistDelete { item });
            assert_eq!(check!($uut => &[$($s),*]), expected);
        };
        ($uut:expr => Some($index:expr), &[$($s:expr),* $(,)?], delete($item:expr)) => {
            let item: &'static str = $item;
            let item = &TestItem(item);
            let expected = Some(Cmd::PlaylistDelete { item });
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
    }
}
