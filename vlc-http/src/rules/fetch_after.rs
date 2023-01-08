// Copyright (C) 2021-2023  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use super::{need::Need, Action, Command, PlaybackStatus, PlaylistInfo, Rule, Time};
use shared::TimeDifference;

pub(super) trait FetchAfterSpec<T>: Send + Sync + std::fmt::Debug
where
    T: Send + Sync + PartialEq + std::fmt::Debug,
{
    fn info_from_playback(&self, _playback: &PlaybackStatus) -> Option<T> {
        None
    }
    fn info_from_playlist(&self, _playlist: &PlaylistInfo) -> Option<T> {
        None
    }
    fn is_trigger(&self, command: &Command, info: Option<&T>) -> bool;
    fn gen_action(&self) -> Action;
    fn allowed_delay_millis(&self) -> u32 {
        50
    }
}
#[derive(Debug)]
pub(super) struct FetchAfterRule<T, S>
where
    T: Send + Sync + PartialEq + std::fmt::Debug,
    S: FetchAfterSpec<T>,
{
    info_time: Option<(T, Time)>,
    cmd_time: Option<Time>,
    spec: S,
}
impl<T, S> FetchAfterRule<T, S>
where
    T: Send + Sync + PartialEq + std::fmt::Debug,
    S: FetchAfterSpec<T>,
{
    pub fn from_spec(spec: S) -> Self {
        Self {
            info_time: None,
            cmd_time: None,
            spec,
        }
    }
    fn notify_info(&mut self, info: T, time: Time) {
        self.info_time = match self.info_time.take() {
            Some((prev_info, prev_time)) if prev_info == info => Some((prev_info, prev_time)),
            _ => Some((info, time)),
        };
    }
    fn info(&self) -> Option<&T> {
        self.info_time.as_ref().map(|(info, _)| info)
    }
}
impl<T, S> Rule for FetchAfterRule<T, S>
where
    T: Send + Sync + PartialEq + std::fmt::Debug,
    S: FetchAfterSpec<T>,
{
    fn notify_playback(&mut self, playback: &PlaybackStatus) {
        if let Some(info) = self.spec.info_from_playback(playback) {
            self.notify_info(info, playback.received_time);
        }
    }
    fn notify_playlist(&mut self, playlist: &PlaylistInfo) {
        if let Some(info) = self.spec.info_from_playlist(playlist) {
            self.notify_info(info, playlist.received_time);
        }
    }
    fn notify_command(&mut self, now: Time, command: &Command) {
        if self.spec.is_trigger(command, self.info()) {
            self.cmd_time = Some(now);
        }
    }
    fn get_need(&self, now: Time) -> Need {
        match (&self.cmd_time, &self.info_time) {
            (None, _) => None, // never commanded
            (Some(cmd_time), Some((_, change_time))) if cmd_time < change_time => None, // cmd before change
            (Some(cmd_time), _) => {
                let since_cmd = now - *cmd_time;
                let allowed_delay = {
                    let allowed_delay_millis = self.spec.allowed_delay_millis();
                    TimeDifference::milliseconds(i64::from(allowed_delay_millis))
                };
                let delay = allowed_delay - since_cmd;
                let delay = if delay > TimeDifference::zero() {
                    Some(
                        delay
                            .to_std()
                            .expect("positive duration conversion succeeds"),
                    )
                } else {
                    None
                };
                Some((delay, self.spec.gen_action()))
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::super::{
        tests::{immediate, some_millis, time, PlaylistItem},
        PlaybackTiming,
    };
    use super::*;

    #[derive(Debug)]
    struct DurationPauseSpec;
    impl FetchAfterSpec<Option<()>> for DurationPauseSpec {
        fn info_from_playback(&self, playback: &PlaybackStatus) -> Option<Option<()>> {
            Some(if playback.timing.duration_secs == 0 {
                None
            } else {
                Some(())
            })
        }
        fn is_trigger(&self, command: &Command, _: Option<&Option<()>>) -> bool {
            matches!(command, Command::PlaybackPause)
        }
        fn gen_action(&self) -> Action {
            Action::fetch_playback_status()
        }
    }
    #[derive(Debug)]
    struct ItemsStopSpec;
    impl FetchAfterSpec<usize> for ItemsStopSpec {
        fn info_from_playlist(&self, playlist: &PlaylistInfo) -> Option<usize> {
            Some(playlist.items.len())
        }
        fn is_trigger(&self, command: &Command, _: Option<&usize>) -> bool {
            matches!(command, Command::PlaybackStop)
        }
        fn gen_action(&self) -> Action {
            Action::fetch_playlist_info()
        }
    }

    fn dummy_playlist_item() -> PlaylistItem {
        PlaylistItem {
            duration_secs: None,
            id: String::new(),
            name: String::new(),
            url: url::Url::parse("file:///").expect("valid url"),
        }
    }

    #[test]
    fn fetch_after_rule_sets_change_time() {
        {
            //PlaybackStatus
            let mut far = FetchAfterRule::from_spec(DurationPauseSpec);
            let t = |(_, t)| t;
            assert_eq!(far.info_time.map(t), None);
            // notify [first] (t=0)
            far.notify_playback(&PlaybackStatus::default());
            assert_eq!(far.info_time.map(t), Some(time(0)));
            // notify [info change] (t=1)
            far.notify_playback(&PlaybackStatus {
                timing: PlaybackTiming {
                    duration_secs: 1, // -> Some(())
                    ..PlaybackTiming::default()
                },
                received_time: time(1),
                ..PlaybackStatus::default()
            });
            assert_eq!(far.info_time.map(t), Some(time(1)));
            // notify [identical] (t=1, still)
            far.notify_playback(&PlaybackStatus {
                timing: PlaybackTiming {
                    duration_secs: 20, // -> Some(())
                    ..PlaybackTiming::default()
                },
                received_time: time(3),
                ..PlaybackStatus::default()
            });
            assert_eq!(far.info_time.map(t), Some(time(1)));
            // notify [info change] (t=5)
            far.notify_playback(&PlaybackStatus {
                timing: PlaybackTiming {
                    duration_secs: 0, // -> None
                    ..PlaybackTiming::default()
                },
                received_time: time(5),
                ..PlaybackStatus::default()
            });
            assert_eq!(far.info_time.map(t), Some(time(5)));
        }
        {
            //PlaylistInfo
            let mut far = FetchAfterRule::from_spec(ItemsStopSpec);
            let t = |(_, t)| t;
            assert_eq!(far.info_time.map(t), None);
            // notify [first] (t=0)
            far.notify_playlist(&PlaylistInfo::default());
            assert_eq!(far.info_time.map(t), Some(time(0)));
            // notify [info change] (t=1)
            far.notify_playlist(&PlaylistInfo {
                received_time: time(1),
                items: vec![dummy_playlist_item()],
            });
            assert_eq!(far.info_time.map(t), Some(time(1)));
            // notify [identical] (t=1, still)
            far.notify_playlist(&PlaylistInfo {
                received_time: time(3),
                items: vec![dummy_playlist_item()],
            });
            assert_eq!(far.info_time.map(t), Some(time(1)));
            // notify [info change] (t=5)
            far.notify_playlist(&PlaylistInfo {
                received_time: time(5),
                ..PlaylistInfo::default()
            });
            assert_eq!(far.info_time.map(t), Some(time(5)));
        }
    }
    #[test]
    fn fetch_after_rule_captures_cmd() {
        let mut far = FetchAfterRule::from_spec(DurationPauseSpec);
        // default None
        assert_eq!(far.cmd_time, None);
        // volume commands
        far.notify_command(time(1), &Command::PlaybackPause);
        assert_eq!(far.cmd_time, Some(time(1)));
        far.notify_command(time(2), &Command::PlaybackPause);
        assert_eq!(far.cmd_time, Some(time(2)));
        let url = url::Url::parse("file:///some_url").expect("url parses");
        // ignores non-volume commands
        let ignored_cmds = &[
            Command::PlaylistAdd { url },
            Command::PlaylistPlay {
                item_id: Some("id".to_string()),
            },
            Command::PlaybackResume,
            // * Command::PlaybackPause,
            Command::PlaybackStop,
            Command::SeekNext,
            Command::SeekPrevious,
            Command::SeekTo { seconds: 20 },
            Command::SeekRelative { seconds_delta: -20 },
            Command::Volume { percent: 99 },
            Command::VolumeRelative { percent_delta: -5 },
            Command::PlaybackSpeed { speed: 0.7 },
        ];
        for ignored_cmd in ignored_cmds {
            far.notify_command(time(3), ignored_cmd); // ignore-cmd at t=3
            assert_eq!(far.cmd_time, Some(time(2))); // unchanged (t=2)
        }
    }
    #[test]
    fn fetch_after_rule_gets_need() {
        #[derive(Debug)]
        struct NeverSpec(bool);
        impl FetchAfterSpec<()> for NeverSpec {
            fn is_trigger(&self, _command: &Command, _: Option<&()>) -> bool {
                false
            }
            fn gen_action(&self) -> Action {
                if self.0 {
                    Action::fetch_playlist_info()
                } else {
                    Action::fetch_playback_status()
                }
            }
        }
        let allowed_delay_millis = u64::from(NeverSpec(false).allowed_delay_millis());

        let params: [(bool, Box<dyn Fn() -> Action>); 2] = [
            (false, Box::new(Action::fetch_playback_status)),
            (true, Box::new(Action::fetch_playlist_info)),
        ];
        for (spec_arg, action_fn) in params {
            let mut far = FetchAfterRule::from_spec(NeverSpec(spec_arg));
            // default -> None
            assert_eq!(far.get_need(time(0)), None);
            far.info_time = None;
            assert_eq!(far.get_need(time(0)), None);
            // no cmd_time -> None
            far.cmd_time = None;
            for t in 0..10 {
                far.info_time = Some(((), time(t)));
                assert_eq!(far.get_need(time(100)), None);
            }
            // cmd_time only, no change time
            far.cmd_time = Some(time(0));
            far.info_time = None;
            assert_eq!(far.get_need(time(100)), immediate(action_fn()));
            // manually activate (tie!)
            far.info_time = Some(((), time(1)));
            far.cmd_time = Some(time(1));
            assert_eq!(far.get_need(time(2)), immediate(action_fn()));
            assert_eq!(
                far.get_need(time(1)),
                some_millis(allowed_delay_millis, action_fn())
            );
            assert_eq!(
                far.get_need(time(0)),
                some_millis(1000 + allowed_delay_millis, action_fn())
            );
            // manually activate (cmd after change)
            far.info_time = Some(((), time(0)));
            far.cmd_time = Some(time(1));
            assert_eq!(far.get_need(time(2)), immediate(action_fn()));
            assert_eq!(
                far.get_need(time(1)),
                some_millis(allowed_delay_millis, action_fn())
            );
            assert_eq!(
                far.get_need(time(0)),
                some_millis(1000 + allowed_delay_millis, action_fn())
            );
        }
    }
}
