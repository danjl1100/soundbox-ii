// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use super::{Action, PlaybackStatus, PlaylistInfo, Rules};
use shared::time_now;

pub fn time(secs: i64) -> shared::Time {
    shared::time_from_secs_opt(secs).expect("valid seconds input in test")
}

#[tokio::test]
async fn rules_initialize_status() {
    let start = time_now();
    let mut rules = Rules::new();
    assert_eq!(
        rules.next_action(time(0)).await,
        Some(Action::fetch_playback_status())
    );
    rules.notify_playback(&PlaybackStatus::default());
    assert_eq!(
        rules.next_action(time(0)).await,
        Some(Action::fetch_playlist_info())
    );
    rules.notify_playlist(&PlaylistInfo::default());
    assert_eq!(rules.next_action(time(0)).await, None); // TODO: remove me when the ongoing fetch action is added
    let end = time_now();
    let duration = end - start;
    assert!(duration.num_seconds() < 2);
}
