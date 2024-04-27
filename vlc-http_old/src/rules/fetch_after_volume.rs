// Copyright (C) 2021-2023  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use super::{Action, Command, FetchAfterSpec, PlaybackStatus};

#[derive(Debug)]
pub(super) struct FetchAfterVolume;
impl FetchAfterSpec<u16> for FetchAfterVolume {
    fn info_from_playback(&self, playback: &PlaybackStatus) -> Option<u16> {
        Some(playback.volume_percent)
    }
    fn is_trigger(&self, command: &Command, volume_percent: Option<&u16>) -> bool {
        match (command, volume_percent) {
            (Command::VolumeRelative { percent_delta }, Some(volume_percent))
                if *percent_delta < 0 && *volume_percent == 0 =>
            {
                false
            }
            (Command::Volume { .. } | Command::VolumeRelative { .. }, _) => true,
            _ => false,
        }
    }
    fn gen_action(&self) -> Action {
        Action::fetch_playback_status()
    }
}

#[cfg(test)]
#[allow(clippy::bool_assert_comparison)]
mod tests {
    use super::*;

    #[test]
    fn fetch_after_volume_gets_info() {
        let fav = FetchAfterVolume;
        assert_eq!(fav.info_from_playback(&PlaybackStatus::default()), Some(0));
        assert_eq!(
            fav.info_from_playback(&PlaybackStatus {
                volume_percent: 50,
                ..PlaybackStatus::default()
            }),
            Some(50)
        );
        assert_eq!(
            fav.info_from_playback(&PlaybackStatus {
                volume_percent: 50,
                ..PlaybackStatus::default()
            }),
            Some(50)
        );
        assert_eq!(
            fav.info_from_playback(&PlaybackStatus {
                volume_percent: 100,
                ..PlaybackStatus::default()
            }),
            Some(100)
        );
    }
    #[test]
    fn fetch_after_volume_triggers_on_cmd() {
        let fav = FetchAfterVolume;
        assert!(fav.is_trigger(&Command::Volume { percent: 20 }, None));
        assert!(fav.is_trigger(&Command::VolumeRelative { percent_delta: -30 }, None));
        let url = url::Url::parse("file:///some_url").expect("url parses");
        // ignores non-volume commands
        let ignored_cmds = &[
            Command::PlaylistAdd { url },
            Command::PlaylistPlay {
                item_id: Some("id".to_string()),
            },
            Command::PlaybackResume,
            Command::PlaybackPause,
            Command::PlaybackStop,
            Command::SeekNext,
            Command::SeekPrevious,
            Command::SeekTo { seconds: 20 },
            Command::SeekRelative { seconds_delta: -20 },
            // * Command::Volume { percent: 99 },
            // * Command::VolumeRelative { percent_delta: -5 },
            Command::PlaybackSpeed { speed: 0.7 },
        ];
        let dummy_state = 0;
        for ignored_cmd in ignored_cmds {
            assert_eq!(fav.is_trigger(ignored_cmd, None), false);
            assert_eq!(fav.is_trigger(ignored_cmd, Some(&dummy_state)), false);
        }
    }
    #[test]
    fn fetch_after_volume_gens_need() {
        let fav = FetchAfterVolume;
        assert_eq!(fav.gen_action(), Action::fetch_playback_status());
    }
    #[test]
    fn fetch_after_volume_ignores_below_zero() {
        let fav = FetchAfterVolume;
        let down_command = Command::VolumeRelative { percent_delta: -1 };
        let state_default = 100;
        let state_0 = 0;
        assert_eq!(fav.is_trigger(&down_command, None), true);
        assert_eq!(fav.is_trigger(&down_command, Some(&state_default)), true);
        assert_eq!(fav.is_trigger(&down_command, Some(&state_0)), false);
        let down_command_more = Command::VolumeRelative { percent_delta: -19 };
        assert_eq!(fav.is_trigger(&down_command_more, Some(&state_0)), false);
    }
}
