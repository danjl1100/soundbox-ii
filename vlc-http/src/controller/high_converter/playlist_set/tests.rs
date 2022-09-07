// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use crate::{
    command::LowCommand,
    controller::high_converter::{ConverterIterator, LowAction},
    vlc_responses::{PlaybackInfo, PlaybackStatus, PlaylistInfo, PlaylistItem},
};

use super::{Command, Converter, SourceUrlType};

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
        .map(|(id, url)| playlist_item_with_id_url(&id, &file_url(url)))
        .collect()
}
fn playlist_item_with_id_url<T>(id: &T, url: &url::Url) -> PlaylistItem
where
    T: ToString,
{
    PlaylistItem {
        duration_secs: None,
        id: id.to_string(),
        name: String::default(),
        url: url.to_string(),
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
pub(super) fn calc_current_item_index<'a>(
    items: &'a [PlaylistItem],
    current_url: &Option<String>,
) -> Option<(usize, &'a PlaylistItem)> {
    current_url.as_ref().and_then(|current_url| {
        items
            .iter()
            .enumerate()
            .find(|(_, item)| (item.url == *current_url))
    })
}

pub(super) fn converter_permutations() -> impl Iterator<Item = (SourceUrlType, Converter)> {
    SourceUrlType::permutations().map(|ty| {
        let initial_flag = match ty {
            SourceUrlType::Current(sentinel) => Some(sentinel),
            SourceUrlType::Next => None,
        };
        let mut converter = Converter::new();
        converter.keep_unplayed_added_current = initial_flag;
        (ty, converter)
    })
}

/// Debug-coercion for `PlaylistItem`s to be simple "id => url" pairs
#[derive(PartialEq, Eq)]
struct ItemsFmt<'a>(&'a [PlaylistItem]);
impl<'a> std::fmt::Debug for ItemsFmt<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_map()
            .entries(
                self.0
                    .iter()
                    .map(|item| (parse_id(item), item.url.to_string())),
            )
            .finish()
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
            inner => write!(f, "{:?}", inner),
        }
    }
}

struct TestHarness {
    converter: Converter,
    command: Command,
    playlist_items: Vec<PlaylistItem>,
    playback_current_id: Option<u64>,
    // marker to ensure `assert_done` is called
    pending_done_check: Option<()>,
}
impl TestHarness {
    fn new(command: Command) -> Self {
        let converter = Converter::new();
        Self {
            converter,
            command,
            playlist_items: vec![],
            playback_current_id: None,
            pending_done_check: Some(()),
        }
    }
    fn update_for_cmd(&mut self, command: LowCommand) {
        match command {
            LowCommand::PlaylistAdd { url } => {
                let id = next_id(&self.playlist_items);
                self.playlist_items
                    .push(playlist_item_with_id_url(&id, &url));
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
                    .find_map(|(idx, item)| (item.id == item_id).then_some(idx))
                {
                    self.playlist_items.remove(index);
                }
                match self.playback_current_id {
                    Some(id) if id.to_string() == item_id => {
                        self.playback_current_id.take();
                    }
                    _ => {}
                }
            }
            cmd => unimplemented!("{cmd:?}"),
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
        let result = self.converter.next((&status, &playlist), &self.command);
        println!(
            "result {result:?} for current_id {id:?}, playlist {items:#?}",
            result = ResultFmt(&result),
            id = &self.playback_current_id,
            items = ItemsFmt(&self.playlist_items),
        );
        // assert RESULT
        assert_eq!(ResultFmt(&result), ResultFmt(&expected_result));
        match result {
            Ok(()) => {}
            Err(LowAction::Command(cmd)) => self.update_for_cmd(cmd),
            Err(action) => unimplemented!("{action:?}"),
        }
        // assert PLAYLIST ITEMS
        {
            let self_items = ItemsFmt(&self.playlist_items);
            let expected_items = ItemsFmt(expected_items);
            assert_eq!(
                self_items, expected_items,
                "\nself_items {self_items:#?}, expected_items {expected_items:#?}"
            );
        }
        // assert PLAYBACK CURRENT ID
        assert_eq!(self.playback_current_id, expected_current_id);
    }
    fn assert_done(mut self, expected_items: &[PlaylistItem], expected_current_id: Option<u64>) {
        for _ in 0..100 {
            self.assert_inner(Ok(()), expected_items, expected_current_id);
        }
        self.pending_done_check.take();
    }
}
impl Drop for TestHarness {
    fn drop(&mut self) {
        assert!(
            self.pending_done_check.is_none(),
            "TestHarness dropped while still pending done check!"
        );
    }
}

#[test]
fn test_harness_empty() {
    let current_str = "current";
    let current = file_url(current_str);
    let mut uut = TestHarness::new(Command {
        current_or_past_url: current.clone(),
        next_urls: vec![],
        max_history_count: 1.try_into().expect("nonzero"),
    });
    //
    let add_current = LowCommand::PlaylistAdd { url: current }.into();
    let play_current = LowCommand::PlaylistPlay {
        item_id: Some(0.to_string()),
    }
    .into();
    let item_current = &items![current_str];
    //
    uut.assert_next(add_current, item_current, None);
    uut.assert_next(play_current, item_current, Some(0));
    uut.assert_done(item_current, Some(0));
}
#[test]
fn no_delete_after_adding_new_items() {
    let current_str = "current";
    let current = file_url(current_str);
    let next_strs = &["next1", "next2", "next3"];
    let next_urls = next_strs.iter().copied().map(file_url).collect();
    let mut uut = TestHarness::new(Command {
        current_or_past_url: current.clone(),
        next_urls,
        max_history_count: 1.try_into().expect("nonzero"),
    });
    //
    let add_current = LowCommand::PlaylistAdd { url: current }.into();
    let play_current = LowCommand::PlaylistPlay {
        item_id: Some(0.to_string()),
    }
    .into();
    let add = |idx| {
        LowCommand::PlaylistAdd {
            url: file_url(next_strs[idx]),
        }
        .into()
    };
    let item_current = &items![current_str];
    let items = &items![current_str; ..next_strs];
    //
    uut.assert_next(add_current, item_current, None);
    uut.assert_next(play_current, item_current, Some(0));
    uut.assert_next(add(0), &items[..2], Some(0));
    uut.assert_next(add(1), &items[..3], Some(0));
    uut.assert_next(add(2), items, Some(0));
    uut.assert_done(items, Some(0));
}
#[test]
fn removes_then_adds_new_items() {
    let existing_items =
        &items![20 => "wrong", 25 => "existing", 30 => "olditems", 35 => "lastoldie"];
    let current_str = "time is now";
    let current = file_url(current_str);
    let next_strs = &["future1", "future2", "future tree"];
    let next_urls = next_strs.iter().copied().map(file_url).collect();
    let mut uut = TestHarness::new(Command {
        current_or_past_url: current,
        next_urls,
        max_history_count: 2.try_into().expect("nonzero"),
    });
    uut.playlist_items = existing_items.clone();
    uut.playback_current_id = Some(30);
    //
    let delete = |id: usize| {
        LowCommand::PlaylistDelete {
            item_id: id.to_string(),
        }
        .into()
    };
    let play = |id: usize| {
        LowCommand::PlaylistPlay {
            item_id: Some(id.to_string()),
        }
        .into()
    };
    let add = |url_str: &str| {
        LowCommand::PlaylistAdd {
            url: file_url(url_str),
        }
        .into()
    };
    //
    let one_deleted = &items![20 => "wrong", 25 => "existing", 35 => "lastoldie"];
    let end_items = &items![
        25 => "existing",
        35 => "lastoldie",
        36 => current_str,
        37 => next_strs[0],
        38 => next_strs[1],
        39 => next_strs[2],
    ];
    // delete current
    uut.assert_next(delete(30), one_deleted, None);
    // delete first (trim to length)
    uut.assert_next(delete(20), &one_deleted[1..], None);
    // add current to end
    uut.assert_next(add(current_str), &end_items[..3], None);
    // play current
    uut.assert_next(play(36), &end_items[..3], Some(36));
    // add nexts to end
    uut.assert_next(add(next_strs[0]), &end_items[..4], Some(36));
    uut.assert_next(add(next_strs[1]), &end_items[..5], Some(36));
    uut.assert_next(add(next_strs[2]), &end_items[..], Some(36));
    // done
    uut.assert_done(end_items, Some(36));
}
