// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use super::{Action, Command, FetchAfterSpec, PlaybackStatus};

#[derive(Debug)]
pub(super) struct FetchAfterSeek;
impl FetchAfterSpec<(u64, Option<u64>)> for FetchAfterSeek {
    fn info_from_playback(&self, playback: &PlaybackStatus) -> Option<(u64, Option<u64>)> {
        let duration = playback.timing.duration_secs;
        let item_id = playback
            .information
            .as_ref()
            .and_then(|info| info.playlist_item_id);
        Some((duration, item_id))
    }
    fn is_trigger(&self, command: &Command, _: Option<&(u64, Option<u64>)>) -> bool {
        matches!(command, Command::SeekNext | Command::SeekPrevious)
    }
    fn gen_action(&self) -> Action {
        Action::fetch_playback_status()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::bool_assert_comparison)]
    use super::super::{tests::PlaybackInfo, PlaybackTiming};
    use super::*;

    #[test]
    fn fetch_after_seek_gets_info() {
        let fas = FetchAfterSeek;
        assert_eq!(
            fas.info_from_playback(&PlaybackStatus::default()),
            Some((0, None))
        );
        assert_eq!(
            fas.info_from_playback(&PlaybackStatus {
                timing: PlaybackTiming {
                    duration_secs: 2,
                    ..PlaybackTiming::default()
                },
                ..PlaybackStatus::default()
            }),
            Some((2, None))
        );
        assert_eq!(
            fas.info_from_playback(&PlaybackStatus {
                timing: PlaybackTiming {
                    duration_secs: 2,
                    ..PlaybackTiming::default()
                },
                information: Some(PlaybackInfo::default()),
                ..PlaybackStatus::default()
            }),
            Some((2, None))
        );
        assert_eq!(
            fas.info_from_playback(&PlaybackStatus {
                timing: PlaybackTiming {
                    duration_secs: 2,
                    ..PlaybackTiming::default()
                },
                information: Some(PlaybackInfo {
                    playlist_item_id: Some(10),
                    ..PlaybackInfo::default()
                }),
                ..PlaybackStatus::default()
            }),
            Some((2, Some(10)))
        );
        assert_eq!(
            fas.info_from_playback(&PlaybackStatus {
                timing: PlaybackTiming {
                    duration_secs: 2,
                    ..PlaybackTiming::default()
                },
                information: Some(PlaybackInfo {
                    playlist_item_id: Some(22),
                    ..PlaybackInfo::default()
                }),
                ..PlaybackStatus::default()
            }),
            Some((2, Some(22)))
        );
    }
    #[test]
    fn fetch_after_seek_triggers_on_cmd() {
        let fas = FetchAfterSeek;
        assert!(fas.is_trigger(&Command::SeekNext, None));
        assert!(fas.is_trigger(&Command::SeekPrevious, None));
        let url = url::Url::parse("file:///some_url").expect("url parses");
        // ignores non-seek commands
        let ignored_cmds = &[
            Command::PlaylistAdd { url },
            Command::PlaylistPlay {
                item_id: Some("id".to_string()),
            },
            Command::PlaybackResume,
            Command::PlaybackPause,
            Command::PlaybackStop,
            // * Command::SeekNext,
            // * Command::SeekPrevious,
            Command::SeekTo { seconds: 20 },
            Command::SeekRelative { seconds_delta: -20 },
            Command::Volume { percent: 99 },
            Command::VolumeRelative { percent_delta: -5 },
            Command::PlaybackSpeed { speed: 0.7 },
        ];
        let dummy_state = (0, None);
        for ignored_cmd in ignored_cmds {
            assert_eq!(fas.is_trigger(ignored_cmd, None), false);
            assert_eq!(fas.is_trigger(ignored_cmd, Some(&dummy_state)), false);
        }
    }
    #[test]
    fn fetch_after_seek_gens_need() {
        let fas = FetchAfterSeek;
        assert_eq!(fas.gen_action(), Action::fetch_playback_status());
    }
}
