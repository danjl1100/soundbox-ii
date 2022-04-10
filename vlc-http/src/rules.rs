use crate::vlc_responses::{PlaybackStatus, PlaybackTiming, PlaylistInfo};
use crate::{Action, LowCommand as Command};
use shared::{Time, TimeDifference};

use need::ord as ord_need;
use need::Need;
mod need;

pub(crate) struct Rules {
    rules: Vec<Box<dyn Rule>>,
}
impl Rules {
    pub fn new() -> Self {
        Self {
            rules: vec![
                Box::new(FillPlayback::default()),
                Box::new(FillPlaylist::default()),
                Box::new(FetchAfterRule::from_spec(FetchAfterSeek)),
                Box::new(FetchAfterRule::from_spec(FetchAfterVolume)),
                Box::new(FetchAfterTrackEnd::default()),
            ],
        }
    }
    pub async fn next_action(&mut self, now: Time) -> Option<Action> {
        let (delay, action) = self.calc_immediate_need(now)?;
        // TODO add tracing
        // dbg!(delay, &action);
        //  (3) sleep (if applicable)
        if let Some(delay) = delay {
            tokio::time::sleep(delay).await;
        }
        //  (4) return that action
        Some(action)
    }
    fn calc_immediate_need(&mut self, now: Time) -> Need {
        //  (1) calculate all needs
        let needs = self.rules.iter().map(move |rule| {
            rule.get_need(now)
            // TODO add tracing
            // dbg!(rule, &need);
        });
        //  (2) pick most-immediate option
        needs.min_by(ord_need).flatten()
    }
    pub fn notify_playback(&mut self, playback: &PlaybackStatus) {
        for rule in &mut self.rules {
            rule.notify_playback(playback);
        }
    }
    pub fn notify_playlist(&mut self, playlist: &PlaylistInfo) {
        for rule in &mut self.rules {
            rule.notify_playlist(playlist);
        }
    }
    pub fn notify_command(&mut self, now: Time, cmd: &Command) {
        for rule in &mut self.rules {
            rule.notify_command(now, cmd);
        }
    }
}

trait Rule: Send + Sync + core::fmt::Debug {
    fn get_need(&self, now: Time) -> Need;
    fn notify_playback(&mut self, _playback: &PlaybackStatus) {}
    fn notify_playlist(&mut self, _playlist: &PlaylistInfo) {}
    fn notify_command(&mut self, _now: Time, _command: &Command) {}
}

#[derive(Default, Debug)]
struct FillPlayback(Option<()>);
impl Rule for FillPlayback {
    fn notify_playback(&mut self, _: &PlaybackStatus) {
        self.0 = Some(());
    }
    fn get_need(&self, _: Time) -> Need {
        if self.0.is_none() {
            Some((None, Action::fetch_playback_status()))
        } else {
            None
        }
    }
}

#[derive(Default, Debug)]
struct FillPlaylist(Option<()>);
impl Rule for FillPlaylist {
    fn notify_playlist(&mut self, _: &PlaylistInfo) {
        self.0 = Some(());
    }
    fn get_need(&self, _: Time) -> Need {
        if self.0.is_none() {
            Some((None, Action::fetch_playlist_info()))
        } else {
            None
        }
    }
}

trait FetchAfterSpec<T>: Send + Sync + std::fmt::Debug
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
struct FetchAfterRule<T, S>
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
    fn from_spec(spec: S) -> Self {
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

#[derive(Debug)]
struct FetchAfterSeek;
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

#[derive(Debug)]
struct FetchAfterVolume;
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

#[derive(Default, Debug)]
struct FetchAfterTrackEnd {
    playback_timing: Option<(PlaybackTiming, Time)>,
}
impl FetchAfterTrackEnd {
    const DELAY_MS: u64 = 500;
}
impl Rule for FetchAfterTrackEnd {
    fn notify_playback(&mut self, playback: &PlaybackStatus) {
        self.playback_timing = Some((playback.timing, playback.received_time));
    }
    fn get_need(&self, now: Time) -> Need {
        use std::time::Duration;
        match self.playback_timing {
            Some((timing, _)) if !timing.state.is_playing() => None,
            Some((timing, received_time)) if timing.duration_secs > 0 => {
                let timing = timing.predict_change(now - received_time);
                let delay = timing
                    .duration_secs
                    .checked_sub(timing.position_secs)
                    .map(Duration::from_secs)
                    .map(|remaining| remaining + Duration::from_millis(Self::DELAY_MS));
                Some((Some(delay?), Action::fetch_playback_status()))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::need::tests::{immediate, some_millis};
    use super::*;
    use crate::vlc_responses::{PlaybackInfo, PlaylistItem};
    use shared::time_from_secs as time;

    // TODO move this test to the end....
    //  or co-locate Rule structs in sub-modules, with their tests?
    //  goals: ease of navigation, and secondary ~200 lines per module guideline
    #[test]
    fn fetch_after_track_end() {
        let mut fate = FetchAfterTrackEnd::default();
        assert_eq!(fate.get_need(time(0)), None);
        // verify Duration=0 never fetches
        fate.notify_playback(&PlaybackStatus {
            ..PlaybackStatus::default()
        });
        assert_eq!(fate.get_need(time(0)), None);
        let mut count_playing = 0; // verify test is not broken
        for state in [
            shared::PlaybackState::Paused,
            shared::PlaybackState::Playing,
        ] {
            if state.is_playing() {
                count_playing += 1;
            }
            fate.notify_playback(&PlaybackStatus {
                timing: PlaybackTiming {
                    duration_secs: 30,
                    state,
                    ..PlaybackTiming::default()
                },
                ..PlaybackStatus::default()
            });
            assert_eq!(
                fate.get_need(time(0)),
                if state.is_playing() {
                    some_millis(
                        30_000 + FetchAfterTrackEnd::DELAY_MS,
                        Action::fetch_playback_status(),
                    )
                } else {
                    None
                }
            );
            fate.notify_playback(&PlaybackStatus {
                timing: PlaybackTiming {
                    duration_secs: 30,
                    position_secs: 25,
                    state,
                    ..PlaybackTiming::default()
                },
                ..PlaybackStatus::default()
            });
            assert_eq!(
                fate.get_need(time(0)),
                if state.is_playing() {
                    some_millis(
                        5_000 + FetchAfterTrackEnd::DELAY_MS,
                        Action::fetch_playback_status(),
                    )
                } else {
                    None
                }
            );
            fate.notify_playback(&PlaybackStatus {
                timing: PlaybackTiming {
                    duration_secs: 30,
                    position_secs: 30,
                    state,
                    ..PlaybackTiming::default()
                },
                ..PlaybackStatus::default()
            });
            assert_eq!(
                fate.get_need(time(0)),
                if state.is_playing() {
                    some_millis(
                        FetchAfterTrackEnd::DELAY_MS,
                        Action::fetch_playback_status(),
                    )
                } else {
                    None
                }
            );
        }
        assert_eq!(count_playing, 1);
    }

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
            id: "".to_string(),
            name: "".to_string(),
            uri: "".to_string(),
        }
    }

    #[test]
    fn fills_playback() {
        let dummy_time = time(0);
        let uut = &mut FillPlayback::default() as &mut dyn Rule;
        // initial -> fetch
        assert_eq!(
            uut.get_need(dummy_time),
            immediate(Action::fetch_playback_status())
        );
        // set -> no action
        let playback = PlaybackStatus::default();
        uut.notify_playback(&playback);
        assert_eq!(uut.get_need(dummy_time), None);
    }
    #[test]
    fn fills_playlist() {
        let dummy_time = time(0);
        let uut = &mut FillPlaylist::default() as &mut dyn Rule;
        // initial -> fetch
        assert_eq!(
            uut.get_need(dummy_time),
            immediate(Action::fetch_playlist_info())
        );
        // set -> no action
        let playlist = PlaylistInfo::default();
        uut.notify_playlist(&playlist);
        assert_eq!(uut.get_need(dummy_time), None);
    }
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
                    ..Default::default()
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
        // ignores non-seek commands
        let ignored_cmds = &[
            Command::PlaylistAdd {
                uri: "some_uri".to_string(),
            },
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
                ..PlaylistInfo::default()
            });
            assert_eq!(far.info_time.map(t), Some(time(1)));
            // notify [identical] (t=1, still)
            far.notify_playlist(&PlaylistInfo {
                received_time: time(3),
                items: vec![dummy_playlist_item()],
                ..PlaylistInfo::default()
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
        // ignores non-volume commands
        let ignored_cmds = &[
            Command::PlaylistAdd {
                uri: "some_uri".to_string(),
            },
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
        // ignores non-volume commands
        let ignored_cmds = &[
            Command::PlaylistAdd {
                uri: "some_uri".to_string(),
            },
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
