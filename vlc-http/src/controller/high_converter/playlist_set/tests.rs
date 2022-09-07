// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use crate::{
    command::LowCommand,
    controller::high_converter::{ConverterIterator, LowAction},
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

struct TestHarness {
    converter: Converter,
    command: Command,
    playlist_items: Vec<PlaylistItem>,
    playback_current_id: Option<u64>,
}
impl TestHarness {
    fn new(command: Command) -> Self {
        let converter = Converter::new();
        Self {
            converter,
            command,
            playlist_items: vec![],
            playback_current_id: None,
        }
    }
    fn update_for_cmd(&mut self, command: LowCommand) {
        match command {
            LowCommand::PlaylistAdd { url } => {
                let next_id = self.playlist_items.len(); // simple, logic doesn't care (right?)
                self.playlist_items
                    .push(playlist_item_with_id_url(&next_id, &url));
            }
            LowCommand::PlaylistPlay { item_id } => {
                use std::str::FromStr;
                self.playback_current_id = item_id.as_ref().map(|id_str| {
                    u64::from_str(id_str).expect("valid u64 in item_id str {command:?}")
                });
            }
            cmd => unimplemented!("{cmd:?}"),
        }
    }
    #[allow(clippy::needless_pass_by_value)]
    fn assert_next(
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
        assert_eq!(result, expected_result);
        match result {
            Ok(()) => {}
            Err(LowAction::Command(cmd)) => self.update_for_cmd(cmd),
            Err(action) => unimplemented!("{action:?}"),
        }
        assert_eq!(self.playlist_items, expected_items);
        assert_eq!(self.playback_current_id, expected_current_id);
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
    uut.assert_next(Err(add_current), item_current, None);
    uut.assert_next(Err(play_current), item_current, Some(0));
    for _ in 0..100 {
        uut.assert_next(Ok(()), item_current, Some(0));
    }
}
#[test]
fn no_delete_after_adding_new_items() {
    let current_str = "current";
    let current = file_url(current_str);
    let next_strs = vec!["next1", "next2", "next3"];
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
    let items = &items![current_str; ..&next_strs];
    //
    uut.assert_next(Err(add_current), item_current, None);
    uut.assert_next(Err(play_current), item_current, Some(0));
    uut.assert_next(Err(add(0)), &items[..2], Some(0));
    uut.assert_next(Err(add(1)), &items[..3], Some(0));
    uut.assert_next(Err(add(2)), items, Some(0));
    uut.assert_next(Ok(()), items, Some(0));
}
#[test]
#[ignore] // TODO
fn removes_then_adds_new_items() {
    // TODO
    todo!()
}
