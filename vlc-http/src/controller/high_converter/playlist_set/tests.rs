// Copyright (C) 2021-2023  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use crate::{
    command::{LowCommand, TextIntent},
    controller::high_converter::{
        playlist_set::{ItemsFmt, ResultFmt},
        ConverterIterator, LowAction,
    },
    vlc_responses::{PlaybackInfo, PlaybackStatus, PlaylistInfo, PlaylistItem},
};

use super::{Command, Converter};

pub(super) fn playlist_items_with_urls(urls: &[&str]) -> Vec<PlaylistItem> {
    playlist_items_with_ids_urls(urls.iter().copied().enumerate())
}
pub(super) fn playlist_items_with_ids_urls<'a, T, U>(ids_urls: T) -> Vec<PlaylistItem>
where
    T: IntoIterator<Item = (U, &'a str)>,
    U: ToString,
{
    ids_urls
        .into_iter()
        .map(|(id, url)| playlist_item_with_id_url(&id, file_url(url)))
        .collect()
}
fn playlist_item_with_id_url<T>(id: &T, url: url::Url) -> PlaylistItem
where
    T: ToString,
{
    PlaylistItem {
        duration_secs: None,
        id: id.to_string(),
        name: String::default(),
        url,
    }
}
fn parse_id(item: &PlaylistItem) -> u64 {
    use std::str::FromStr;
    u64::from_str(&item.id).expect("numeric id")
}
fn next_id(items: &[PlaylistItem]) -> u64 {
    items
        .iter()
        .map(parse_id)
        .max()
        .map(|max| max + 1)
        .unwrap_or_default()
}

pub(super) fn file_url(s: &str) -> url::Url {
    url::Url::parse(&format!("file:///{s}")).expect("url")
}

struct TestHarness {
    converter: Converter,
    command: Command,
    data: TestHarnessData,
    data_published: TestHarnessData,
    // marker to ensure `assert_done` is called
    pending_done_check: Option<()>,
    // ensure monotonicity of playlist items (after deletion, ids are never re-used)
    min_next_id: u64,
}
#[derive(Default)]
struct TestHarnessData {
    playlist_items: Vec<PlaylistItem>,
    playback_current_id: Option<u64>,
}
impl TestHarness {
    fn new(command: Command) -> Self {
        let converter = Converter::new();
        Self {
            converter,
            command,
            data: TestHarnessData::default(),
            data_published: TestHarnessData::default(),
            pending_done_check: Some(()),
            min_next_id: 0,
        }
    }
    fn assert_next(
        &mut self,
        expected_err: LowAction,
        expected_items: &[PlaylistItem],
        expected_current_id: Option<u64>,
    ) {
        self.assert_inner(Err(expected_err), expected_items, expected_current_id);
    }
    #[allow(clippy::needless_pass_by_value)]
    fn assert_inner(
        &mut self,
        expected_result: Result<(), LowAction>,
        expected_items: &[PlaylistItem],
        expected_current_id: Option<u64>,
    ) {
        // construct throw-away `status`, `playlist`
        let (status, playlist) = self.data_published.instantiate_views();
        let result = self.converter.next((&status, &playlist), &self.command);
        // assert RESULT
        assert_eq!(ResultFmt(&result), ResultFmt(&expected_result));
        // update state for RESULT
        match result {
            Ok(()) => {}
            Err(LowAction::Command(cmd)) => {
                let next_id = next_id(&self.data.playlist_items).max(self.min_next_id);
                self.min_next_id = next_id;
                self.data.update_for_cmd(&cmd, next_id);
                self.data.selective_copy_to(cmd, &mut self.data_published);
            }
            Err(LowAction::QueryPlaybackStatus) => {
                self.data.copy_status_to(&mut self.data_published);
            }
            Err(action) => unimplemented!("{action:?}"),
        }
        // assert PLAYLIST ITEMS
        {
            let self_items = ItemsFmt(&self.data_published.playlist_items);
            let expected_items = ItemsFmt(expected_items);
            assert_eq!(
                self_items, expected_items,
                "\nself_items {self_items:#?}, expected_items {expected_items:#?}"
            );
        }
        // assert PLAYBACK CURRENT ID
        assert_eq!(self.data_published.playback_current_id, expected_current_id);
    }
    fn assert_done(mut self, expected_items: &[PlaylistItem], expected_current_id: Option<u64>) {
        for _ in 0..100 {
            self.assert_inner(Ok(()), expected_items, expected_current_id);
        }
        self.pending_done_check.take();
    }
    fn publish_playlist_items(&mut self, items: Vec<PlaylistItem>) {
        self.data.playlist_items = items.clone();
        self.data_published.playlist_items = items;
    }
    fn publish_playback_current_id(&mut self, current_id: Option<u64>) {
        self.data.playback_current_id = current_id;
        self.data_published.playback_current_id = current_id;
    }
}
impl Drop for TestHarness {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            assert!(
                self.pending_done_check.is_none(),
                "TestHarness dropped while still pending done check!"
            );
        }
    }
}
impl TestHarnessData {
    fn instantiate_views(&self) -> (PlaybackStatus, PlaylistInfo) {
        let playlist = PlaylistInfo {
            items: self.playlist_items.clone(),
            ..Default::default()
        };
        let status = PlaybackStatus {
            information: self.playback_current_id.map(|id| PlaybackInfo {
                playlist_item_id: Some(id),
                ..Default::default()
            }),
            ..Default::default()
        };
        (status, playlist)
    }
    fn update_for_cmd(&mut self, command: &LowCommand, next_id: u64) {
        match command {
            LowCommand::PlaylistAdd { url } => {
                self.playlist_items
                    .push(playlist_item_with_id_url(&next_id, url.clone()));
            }
            LowCommand::PlaylistPlay { item_id } => {
                use std::str::FromStr;
                self.playback_current_id = item_id.as_ref().map(|id_str| {
                    u64::from_str(id_str).expect("valid u64 in item_id str {command:?}")
                });
            }
            LowCommand::PlaylistDelete { item_id } => {
                if let Some(index) = self
                    .playlist_items
                    .iter()
                    .enumerate()
                    .find_map(|(idx, item)| (&item.id == item_id).then_some(idx))
                {
                    self.playlist_items.remove(index);
                }
                match self.playback_current_id {
                    Some(id) if &id.to_string() == item_id => {
                        self.playback_current_id.take();
                    }
                    _ => {}
                }
            }
            cmd => unimplemented!("{cmd:?}"),
        }
    }
    fn selective_copy_to(&self, command: LowCommand, dest: &mut Self) {
        let intent = TextIntent::from(command);
        match intent {
            TextIntent::Status(_) => self.copy_status_to(dest),
            TextIntent::Playlist(_) => self.clone_playlist_to(dest),
        }
    }
    fn copy_status_to(&self, dest: &mut Self) {
        dest.playback_current_id = self.playback_current_id;
    }
    fn clone_playlist_to(&self, dest: &mut Self) {
        dest.playlist_items = self.playlist_items.clone();
    }
}
fn make_urls(current_str: &str, next_strs: &[&str]) -> Vec<url::Url> {
    std::iter::once(&current_str)
        .chain(next_strs.iter())
        .copied()
        .map(file_url)
        .collect()
}

fn add(url_str: &str) -> LowAction {
    let url = file_url(url_str);
    LowCommand::PlaylistAdd { url }.into()
}
fn play(id: usize) -> LowAction {
    let item_id = Some(id.to_string());
    LowCommand::PlaylistPlay { item_id }.into()
}
fn delete(id: usize) -> LowAction {
    let item_id = id.to_string();
    LowCommand::PlaylistDelete { item_id }.into()
}

#[test]
fn test_harness_empty() {
    let current = "current";
    let mut uut = TestHarness::new(Command {
        urls: make_urls(current, &[]),
        max_history_count: 1.try_into().expect("nonzero"),
    });
    let item_current = &items![current];
    uut.assert_next(add(current), item_current, None);
    uut.assert_next(play(0), item_current, Some(0));
    uut.assert_done(item_current, Some(0));
}
#[test]
fn deletes_one() {
    let items = items!["a", "b", "Q", "d"];
    let mut uut = TestHarness::new(Command {
        urls: make_urls("c", &["d"]),
        max_history_count: 99.try_into().expect("nonzero"),
    });
    uut.publish_playlist_items(items);
    uut.publish_playback_current_id(Some(2));
    uut.assert_next(add("c"), &items!["a", "b", "Q", "d", "c"], Some(2));
    uut.assert_next(add("d"), &items!["a", "b", "Q", "d", "c", "d"], Some(2));
    uut.assert_next(
        delete(3),
        &items![0=>"a", 1=>"b", 2=>"Q", 4=>"c", 5=>"d"],
        Some(2),
    );
    // NOT THIS: uut.assert_next(delete(3), &items![0=>"a", 1=>"b", 4=>"c", 5=>"d"], Some(2));
    uut.assert_done(&items![0=>"a", 1=>"b", 2=>"Q", 4=>"c", 5=>"d"], Some(2));
}
#[test]
fn no_delete_after_adding_new_items() {
    let current = "current";
    let next_strs = &["next1", "next2", "next3"];
    let urls = make_urls(current, next_strs);
    let mut uut = TestHarness::new(Command {
        urls,
        max_history_count: 1.try_into().expect("nonzero"),
    });
    //
    let play_current = play(0);
    let add_next = |idx: usize| add(next_strs[idx]);
    let item_current = &items![current];
    let items = &items![current; ..next_strs];
    //
    uut.assert_next(add(current), item_current, None);
    uut.assert_next(add_next(0), &items[..2], None);
    uut.assert_next(add_next(1), &items[..3], None);
    uut.assert_next(add_next(2), items, None);
    uut.assert_next(play_current, items, Some(0));
    uut.assert_done(items, Some(0));
}
#[test]
fn removes_then_adds_new_items() {
    let existing_items =
        &items![20 => "wrong", 25 => "existing", 30 => "olditems", 35 => "lastoldie"];
    let current_str = "time is now";
    let next_strs = &["future1", "future2", "future tree"];
    let urls = make_urls(current_str, next_strs);
    let mut uut = TestHarness::new(Command {
        urls,
        max_history_count: 2.try_into().expect("nonzero"),
    });
    uut.publish_playlist_items(existing_items.clone());
    uut.publish_playback_current_id(Some(30));
    //
    let one_deleted = &items![
        25 => "existing",
        30 => "olditems",
        35 => "lastoldie",
        36 => current_str,
        37 => next_strs[0],
        38 => next_strs[1],
        39 => next_strs[2],
    ];
    let end_items = &items![
        25 => "existing",
        30 => "olditems",
        36 => current_str,
        37 => next_strs[0],
        38 => next_strs[1],
        39 => next_strs[2],
    ];
    // delete first (trim to length)
    uut.assert_next(delete(20), &one_deleted[..3], Some(30));
    // add current to end
    uut.assert_next(add(current_str), &one_deleted[..4], Some(30));
    // add nexts to end
    uut.assert_next(add(next_strs[0]), &one_deleted[..5], Some(30));
    uut.assert_next(add(next_strs[1]), &one_deleted[..6], Some(30));
    uut.assert_next(add(next_strs[2]), &one_deleted[..], Some(30));
    // delete unmatched
    uut.assert_next(delete(35), end_items, Some(30));
    // done
    uut.assert_done(end_items, Some(30));
}

#[test]
fn keep_current_playing_in_history() {
    // - keep the current-playing item in the history
    // (RATIONALE: it was indeed *started*, so no need to cancel)
    let urls = make_urls("current", &["next1", "next2", "next3"]);
    let mut uut = TestHarness::new(Command {
        urls,
        max_history_count: 99.try_into().expect("nonzero"),
    });
    uut.publish_playlist_items(items!["old1", "old2", "old3"]);
    uut.publish_playback_current_id(Some(2));
    let final_items = items!["old1", "old2", "old3", "current", "next1", "next2", "next3"];
    uut.assert_next(add("current"), &final_items[..4], Some(2));
    uut.assert_next(add("next1"), &final_items[..5], Some(2));
    uut.assert_next(add("next2"), &final_items[..6], Some(2));
    uut.assert_next(add("next3"), &final_items[..], Some(2));
    uut.assert_done(&final_items, Some(2));
}

#[test]
fn handles_changing_current_item() {
    // - accept the reality that current-playing item may change quickly at any point
    //    (e.g. need tests to verify correct behavior when current advances by 1-5 tracks per cmd)
    let urls = make_urls("a", &["b", "c", "d", "e", "f", "g", "h", "i", "j"]);
    let mut uut = TestHarness::new(Command {
        urls,
        max_history_count: 99.try_into().expect("nonzero"),
    });
    uut.publish_playlist_items(items!["0", "1", "2", "3", "4", "5"]);
    uut.publish_playback_current_id(Some(5));
    let final_items =
        items!["0", "1", "2", "3", "4", "5", "a", "b", "c", "d", "e", "f", "g", "h", "i", "j"];
    uut.assert_next(add("a"), &final_items[..7], Some(5));
    uut.publish_playback_current_id(Some(4));
    uut.assert_next(add("b"), &final_items[..8], Some(4));
    uut.publish_playback_current_id(Some(3));
    uut.assert_next(add("c"), &final_items[..9], Some(3));
    uut.publish_playback_current_id(Some(2));
    uut.assert_next(add("d"), &final_items[..10], Some(2));
    uut.publish_playback_current_id(Some(1));
    uut.assert_next(add("e"), &final_items[..11], Some(1));
    uut.publish_playback_current_id(Some(0));
    uut.assert_next(add("f"), &final_items[..12], Some(0));
    uut.publish_playback_current_id(Some(1));
    uut.assert_next(add("g"), &final_items[..13], Some(1));
    uut.publish_playback_current_id(Some(10));
    uut.assert_next(add("h"), &final_items[..14], Some(10));
    uut.publish_playback_current_id(Some(12));
    uut.assert_next(add("i"), &final_items[..15], Some(12));
    uut.publish_playback_current_id(Some(14));
    uut.assert_next(add("j"), &final_items[..], Some(14));
    uut.assert_done(&final_items[..], Some(14));
}
#[test]
fn handles_shortened_items_list() {
    // - accept the reality that items may quickly change at any point
    let urls = make_urls("a", &[]);
    let mut uut = TestHarness::new(Command {
        urls,
        max_history_count: 99.try_into().expect("nonzero"),
    });
    uut.publish_playlist_items(items!["0", "1", "a"]);
    uut.publish_playback_current_id(Some(0));
    uut.assert_next(delete(1), &items![0=>"0", 2=>"a"], Some(0));
    // clear playlist
    let empty_items = items![];
    uut.publish_playlist_items(empty_items);
    uut.publish_playback_current_id(None);
    // verify operation continues (with no panic)
    let final_items = &items![3=>"a"];
    uut.assert_next(add("a"), final_items, None);
    uut.assert_next(play(3), final_items, Some(3));
    uut.assert_done(final_items, Some(3));
}
#[test]
fn never_plays_last_item_far_far_away() {
    // - NEVER play the `last` item, it may be far, far after the ComparisonStart point
    //    (thereby adding false entries to the history)
    let urls = make_urls("a", &["b", "a", "b", "a", "b"]);
    let mut uut = TestHarness::new(Command {
        urls,
        max_history_count: 99.try_into().expect("nonzero"),
    });
    uut.publish_playlist_items(items!["a", "b", "c", "c", "c", "c", "c"]);
    uut.publish_playback_current_id(Some(2));
    let max_items = items!["a", "b", "c", "c", "c", "c", "c", "a", "b", "a", "b", "a", "b"];
    uut.assert_next(add("a"), &max_items[..8], Some(2));
    uut.assert_next(add("b"), &max_items[..9], Some(2));
    uut.assert_next(add("a"), &max_items[..10], Some(2));
    uut.assert_next(add("b"), &max_items[..11], Some(2));
    uut.assert_next(add("a"), &max_items[..12], Some(2));
    uut.assert_next(add("b"), &max_items[..], Some(2));
    uut.assert_next(
        delete(3),
        &items![0=>"a", 1=>"b", 2=>"c", 4=>"c", 5=>"c", 6=>"c", //
            7=>"a", 8=>"b", 9=>"a", 10=>"b", 11=>"a", 12=>"b"],
        Some(2),
    );
    uut.assert_next(
        delete(4),
        &items![0=>"a", 1=>"b", 2=>"c", 5=>"c", 6=>"c", //
            7=>"a", 8=>"b", 9=>"a", 10=>"b", 11=>"a", 12=>"b"],
        Some(2),
    );
    uut.assert_next(
        delete(5),
        &items![0=>"a", 1=>"b", 2=>"c", 6=>"c", //
            7=>"a", 8=>"b", 9=>"a", 10=>"b", 11=>"a", 12=>"b"],
        Some(2),
    );
    uut.assert_next(
        delete(6),
        &items![0=>"a", 1=>"b", 2=>"c", //
            7=>"a", 8=>"b", 9=>"a", 10=>"b", 11=>"a", 12=>"b"],
        Some(2),
    );
    uut.assert_done(
        &items![0=>"a", 1=>"b", 2=>"c", //
            7=>"a", 8=>"b", 9=>"a", 10=>"b", 11=>"a", 12=>"b"],
        Some(2),
    );
}
#[test]
fn never_deletes_after_comparison_starts() {
    // - NEVER delete additional history after establishing a ComparisonStart point
    let urls = make_urls("a", &["b", "c"]);
    let mut uut = TestHarness::new(Command {
        urls,
        max_history_count: 1.try_into().expect("nonzero"),
    });
    uut.publish_playlist_items(items!["0", "1"]);
    uut.publish_playback_current_id(Some(0));
    //
    let extra_items = items!["0", "1", "1", "1", "1", "a", "b", "c"];
    let final_items = items![0=>"0", 2=>"a", 3=>"b", 4=>"c"];
    uut.assert_next(add("a"), &items!["0", "1", "a"], Some(0));
    uut.publish_playlist_items(items!["0", "1", "1", "1", "1", "a"]);
    uut.assert_next(add("b"), &extra_items[..7], Some(0));
    uut.assert_next(add("c"), &extra_items[..], Some(0));
    uut.publish_playlist_items(items!["0", "1", "a", "b", "c"]);
    uut.assert_next(delete(1), &final_items, Some(0));
    uut.assert_done(&final_items, Some(0));
}
