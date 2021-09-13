use crate::{Action, Command, PlaybackStatus, PlaylistInfo};
use shared::{Time, TimeDifference};
use std::cmp::Ordering;
use std::time::Duration;

type Need = Option<(Option<Duration>, Action)>;
fn ord_need(lhs: &Need, rhs: &Need) -> Ordering {
    use Ordering::{Equal, Greater, Less};
    match (lhs, rhs) {
        (None, None) => Equal,
        (Some(_), None) => Less, // Some(need) is always sooner
        (None, Some(_)) => Greater,
        (Some(lhs), Some(rhs)) => match (lhs, rhs) {
            ((None, _), (None, _)) => Equal,
            ((Some(_), _), (None, _)) => Greater, // Some(duration) is always LATER! than no delay
            ((None, _), (Some(_), _)) => Less,
            ((Some(lhs), _), (Some(rhs), _)) => lhs.cmp(rhs),
        },
    }
}

pub(crate) struct Rules {
    rules: Vec<Box<dyn Rule>>,
}
impl Rules {
    pub fn new() -> Self {
        Self {
            rules: vec![
                Box::new(FillPlayback::default()),
                Box::new(FillPlaylist::default()),
                Box::new(FetchAfterSeek::default()),
            ],
        }
    }
    pub async fn next_action(&mut self, now: Time) -> Option<Action> {
        let (delay, action) = self.calc_immediate_need(now)?;
        dbg!(delay, &action);
        //  (3) sleep (if applicable)
        if let Some(delay) = delay {
            tokio::time::sleep(delay).await;
        }
        //  (4) return that action
        Some(action)
    }
    fn calc_immediate_need(&mut self, now: Time) -> Need {
        //  (1) calculate all needs
        let needs = self.rules.iter().map(move |rule| rule.get_need(now));
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

trait Rule: Send + Sync {
    fn get_need(&self, now: Time) -> Need;
    fn notify_playback(&mut self, _playback: &PlaybackStatus) {}
    fn notify_playlist(&mut self, _playlist: &PlaylistInfo) {}
    fn notify_command(&mut self, _now: Time, _command: &Command) {}
}

#[derive(Default)]
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

#[derive(Default)]
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

trait FetchAfterSpec<T>: Send + Sync
where
    T: Send + Sync + PartialEq,
{
    fn info_from_playback(&self, _playback: &PlaybackStatus) -> Option<T> {
        None
    }
    fn info_from_playlist(&self, _playlist: &PlaylistInfo) -> Option<T> {
        None
    }
    fn is_trigger(&self, command: &Command) -> bool;
    fn gen_action(&self) -> Action;
    fn allowed_delay_millis(&self) -> u32 {
        1000
    }
}
struct FetchAfterRule<T, S>
where
    T: Send + Sync + PartialEq,
    S: FetchAfterSpec<T>,
{
    change_time: Option<Time>, //TODO: combine to Option<(T, Time)>, since never have ChangeTime=Some when Info=None
    info: Option<T>,
    cmd_time: Option<Time>,
    spec: S,
}
impl<T, S> FetchAfterRule<T, S>
where
    T: Send + Sync + PartialEq,
    S: FetchAfterSpec<T>,
{
    fn from_spec(spec: S) -> Self {
        Self {
            change_time: None,
            info: None,
            cmd_time: None,
            spec,
        }
    }
    fn notify_info(&mut self, info: T, info_time: Time) {
        self.change_time = match self.info.take() {
            Some(prev_info) if prev_info == info => self.change_time,
            _ => Some(info_time),
        };
        self.info = Some(info);
    }
}
impl<T, S> Rule for FetchAfterRule<T, S>
where
    T: Send + Sync + PartialEq,
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
        if self.spec.is_trigger(command) {
            self.cmd_time = Some(now);
        }
    }
    fn get_need(&self, now: Time) -> Need {
        match (&self.cmd_time, &self.change_time) {
            (None, _) => None, // never commanded
            (Some(cmd_time), Some(change_time)) if cmd_time < change_time => None, // cmd before change
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

#[derive(Default, Debug)]
struct FetchAfterSeek {
    change_time: Option<Time>,
    item_info: Option<(u64, Option<u64>)>,
    seek_time: Option<Time>,
}
impl FetchAfterSeek {
    const DELAY_MILLIS: u64 = 50;
}
impl Rule for FetchAfterSeek {
    fn notify_playback(&mut self, playback: &PlaybackStatus) {
        let item_time = playback.received_time;
        let item_info = {
            let duration = playback.duration;
            let item_id = playback
                .information
                .as_ref()
                .and_then(|info| info.playlist_item_id);
            (duration, item_id)
        };
        self.change_time = match self.item_info.take() {
            Some(prev_item_info) if prev_item_info == item_info => self.change_time,
            _ => Some(item_time),
        };
        self.item_info = Some(item_info);
    }
    fn get_need(&self, now: Time) -> Need {
        match (&self.seek_time, &self.change_time) {
            (None, _) => None, // never seeked
            (Some(seek_time), Some(change_time)) if seek_time < change_time => None, // seek before change
            (Some(seek_time), _) => {
                let since_seek = now - *seek_time;
                let allowed_delay = {
                    let allowed_delay = Duration::from_millis(Self::DELAY_MILLIS);
                    TimeDifference::from_std(allowed_delay).expect("millis within bounds")
                };
                let delay = allowed_delay - since_seek;
                let delay = if delay > TimeDifference::zero() {
                    Some(
                        delay
                            .to_std()
                            .expect("positive duration conversion succeeds"),
                    )
                } else {
                    None
                };
                Some((delay, Action::fetch_playback_status()))
            }
        }
    }
    fn notify_command(&mut self, now: Time, command: &Command) {
        match command {
            Command::SeekNext | Command::SeekPrevious => {
                self.seek_time = Some(now);
            }
            _ => {}
        }
    }
}

#[derive(Default)]
struct FetchAfterVolume {
    change_time: Option<Time>,
    volume: Option<u16>,
    cmd_time: Option<Time>,
}
impl FetchAfterVolume {
    const DELAY_MILLIS: u64 = 500;
}
impl Rule for FetchAfterVolume {
    fn notify_playback(&mut self, playback: &PlaybackStatus) {
        let volume = playback.volume_percent;
        let received_time = playback.received_time;
        self.change_time = match self.volume {
            Some(prev_vol) if prev_vol == volume => self.change_time,
            _ => Some(received_time),
        };
        self.volume = Some(volume);
    }
    fn get_need(&self, now: Time) -> Need {
        match (&self.cmd_time, &self.change_time) {
            (None, _) => None, // no command
            (Some(cmd_time), Some(change_time)) if cmd_time < change_time => None, // command before change
            (Some(cmd_time), _) => {
                let since_cmd = now - *cmd_time;
                let allowed_delay = {
                    let allowed_delay = Duration::from_millis(Self::DELAY_MILLIS);
                    TimeDifference::from_std(allowed_delay).expect("millis within bounds")
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
                Some((delay, Action::fetch_playback_status()))
            }
        }
    }
    fn notify_command(&mut self, now: Time, command: &Command) {
        match command {
            Command::Volume { .. } | Command::VolumeRelative { .. } => {
                self.cmd_time = Some(now);
            }
            _ => {}
        }
    }
}

// TODO!  delay until end of song, then update status!  (minimum delay of 1.0 second)
// struct FetchAfterTrackEnd {
//     todo: (),
// }
// impl Rule for FetchAfterTrackEnd {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vlc_responses::{PlaybackInfo, PlaylistItem};
    use shared::time_from_secs as time;

    struct DurationPauseSpec;
    impl FetchAfterSpec<Option<()>> for DurationPauseSpec {
        fn info_from_playback(&self, playback: &PlaybackStatus) -> Option<Option<()>> {
            Some(if playback.duration == 0 {
                None
            } else {
                Some(())
            })
        }
        fn is_trigger(&self, command: &Command) -> bool {
            matches!(command, Command::PlaybackPause)
        }
        fn gen_action(&self) -> Action {
            Action::fetch_playback_status()
        }
    }
    struct ItemsStopSpec;
    impl FetchAfterSpec<usize> for ItemsStopSpec {
        fn info_from_playlist(&self, playlist: &PlaylistInfo) -> Option<usize> {
            Some(playlist.items.len())
        }
        fn is_trigger(&self, command: &Command) -> bool {
            matches!(command, Command::PlaybackStop)
        }
        fn gen_action(&self) -> Action {
            Action::fetch_playlist_info()
        }
    }

    fn immediate(action: Action) -> Need {
        Some((None, action))
    }
    fn some_millis(millis: u64, action: Action) -> Need {
        Some((Some(Duration::from_millis(millis)), action))
    }
    fn some_millis_action(millis: u64) -> Need {
        some_millis(millis, Action::fetch_playlist_info())
    }
    fn immediate_action() -> Need {
        immediate(Action::fetch_playlist_info())
    }

    fn dummy_playlist_item() -> PlaylistItem {
        PlaylistItem {
            duration: None,
            id: "".to_string(),
            name: "".to_string(),
            uri: "".to_string(),
        }
    }

    #[test]
    fn sorts_need_before_none() {
        let some_need = some_millis_action(1);
        assert_eq!(ord_need(&some_need, &None), Ordering::Less);
        assert_eq!(ord_need(&None, &some_need), Ordering::Greater);
    }
    #[test]
    fn sorts_need_immediate_before_delay() {
        let now = immediate_action();
        let sooner = some_millis_action(5);
        let later = some_millis_action(50);
        assert_eq!(ord_need(&now, &sooner), Ordering::Less);
        assert_eq!(ord_need(&sooner, &later), Ordering::Less);
        assert_eq!(ord_need(&now, &later), Ordering::Less);
        //
        assert_eq!(ord_need(&sooner, &now), Ordering::Greater);
        assert_eq!(ord_need(&later, &sooner), Ordering::Greater);
        assert_eq!(ord_need(&later, &now), Ordering::Greater);
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
    fn assert_none_initial_cleared(uut: &mut dyn Rule) {
        let time_0 = time(0);
        // initial state -> no output
        assert_eq!(uut.get_need(time_0), None);
    }
    #[test]
    fn fetch_after_seek_sets_change_time() {
        let mut fas = FetchAfterSeek::default();
        assert_eq!(fas.change_time, None);
        // notify [first] (t=0)
        fas.notify_playback(&PlaybackStatus::default());
        assert_eq!(fas.change_time, Some(time(0)));
        // notify [duration 0->1] (t=1)
        fas.notify_playback(&PlaybackStatus {
            received_time: time(1),
            duration: 2,
            ..PlaybackStatus::default()
        });
        assert_eq!(fas.change_time, Some(time(1)));
        // notify [identical] (t=1, still)
        fas.notify_playback(&PlaybackStatus {
            received_time: time(3),
            duration: 2,
            ..PlaybackStatus::default()
        });
        assert_eq!(fas.change_time, Some(time(1)));
        // notify [info None->Some(id=None)] (t=1, still)
        fas.notify_playback(&PlaybackStatus {
            received_time: time(4),
            duration: 2,
            information: Some(PlaybackInfo::default()),
            ..PlaybackStatus::default()
        });
        assert_eq!(fas.change_time, Some(time(1)));
        // notify [id None -> Some(10)] (t=5)
        fas.notify_playback(&PlaybackStatus {
            received_time: time(5),
            duration: 2,
            information: Some(PlaybackInfo {
                playlist_item_id: Some(10),
                ..PlaybackInfo::default()
            }),
            ..PlaybackStatus::default()
        });
        assert_eq!(fas.change_time, Some(time(5)));
        // notify [id Some(10) -> Some(22)] (t=6)
        fas.notify_playback(&PlaybackStatus {
            received_time: time(6),
            duration: 2,
            information: Some(PlaybackInfo {
                playlist_item_id: Some(22),
                ..PlaybackInfo::default()
            }),
            ..PlaybackStatus::default()
        });
        assert_eq!(fas.change_time, Some(time(6)));
    }
    #[test]
    fn fetch_after_seek_captures_cmd() {
        let mut fas = FetchAfterSeek::default();
        // default None
        assert_eq!(fas.seek_time, None);
        // seek commands
        fas.notify_command(time(1), &Command::SeekNext);
        assert_eq!(fas.seek_time, Some(time(1)));
        fas.notify_command(time(2), &Command::SeekPrevious);
        assert_eq!(fas.seek_time, Some(time(2)));
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
        for ignored_cmd in ignored_cmds {
            fas.notify_command(time(3), ignored_cmd); // ignore-cmd at t=3
            assert_eq!(fas.seek_time, Some(time(2))); // unchanged (t=2)
        }
    }
    #[test]
    fn fetch_after_seek_gets_need() {
        let mut fas = FetchAfterSeek::default();
        // default -> None
        assert_eq!(fas.get_need(time(0)), None);
        fas.change_time = None;
        fas.seek_time = None;
        assert_eq!(fas.get_need(time(0)), None);
        // no seek time -> None
        fas.seek_time = None;
        for t in 0..10 {
            fas.change_time = Some(time(t));
            assert_eq!(fas.get_need(time(100)), None);
        }
        // seek time only, no change time
        fas.seek_time = Some(time(0));
        fas.change_time = None;
        assert_eq!(
            fas.get_need(time(100)),
            immediate(Action::fetch_playback_status())
        );
        // manually activate (tie!)
        fas.change_time = Some(time(1));
        fas.seek_time = Some(time(1));
        assert_eq!(
            fas.get_need(time(2)),
            immediate(Action::fetch_playback_status())
        );
        assert_eq!(
            fas.get_need(time(1)),
            some_millis(
                FetchAfterSeek::DELAY_MILLIS,
                Action::fetch_playback_status()
            )
        );
        assert_eq!(
            fas.get_need(time(0)),
            some_millis(
                1000 + FetchAfterSeek::DELAY_MILLIS,
                Action::fetch_playback_status()
            )
        );
    }

    #[test]
    fn fetch_after_rule_sets_change_time() {
        {
            //PlaybackStatus
            let mut far = FetchAfterRule::from_spec(DurationPauseSpec);
            assert_eq!(far.change_time, None);
            // notify [first] (t=0)
            far.notify_playback(&PlaybackStatus::default());
            assert_eq!(far.change_time, Some(time(0)));
            // notify [info change] (t=1)
            far.notify_playback(&PlaybackStatus {
                received_time: time(1),
                duration: 1, // -> Some(())
                ..PlaybackStatus::default()
            });
            assert_eq!(far.change_time, Some(time(1)));
            // notify [identical] (t=1, still)
            far.notify_playback(&PlaybackStatus {
                received_time: time(3),
                duration: 20, // -> Some(())
                ..PlaybackStatus::default()
            });
            assert_eq!(far.change_time, Some(time(1)));
            // notify [info change] (t=5)
            far.notify_playback(&PlaybackStatus {
                received_time: time(5),
                duration: 0, // -> None
                ..PlaybackStatus::default()
            });
            assert_eq!(far.change_time, Some(time(5)));
        }
        {
            //PlaylistInfo
            let mut far = FetchAfterRule::from_spec(ItemsStopSpec);
            assert_eq!(far.change_time, None);
            // notify [first] (t=0)
            far.notify_playlist(&PlaylistInfo::default());
            assert_eq!(far.change_time, Some(time(0)));
            // notify [info change] (t=1)
            far.notify_playlist(&PlaylistInfo {
                received_time: time(1),
                items: vec![dummy_playlist_item()],
                ..PlaylistInfo::default()
            });
            assert_eq!(far.change_time, Some(time(1)));
            // notify [identical] (t=1, still)
            far.notify_playlist(&PlaylistInfo {
                received_time: time(3),
                items: vec![dummy_playlist_item()],
                ..PlaylistInfo::default()
            });
            assert_eq!(far.change_time, Some(time(1)));
            // notify [info change] (t=5)
            far.notify_playlist(&PlaylistInfo {
                received_time: time(5),
                ..PlaylistInfo::default()
            });
            assert_eq!(far.change_time, Some(time(5)));
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
        #[derive(PartialEq)]
        enum Never {}

        struct NeverSpec(bool);
        impl FetchAfterSpec<Never> for NeverSpec {
            fn is_trigger(&self, _command: &Command) -> bool {
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
            far.change_time = None;
            far.cmd_time = None;
            assert_eq!(far.get_need(time(0)), None);
            // no cmd_time -> None
            far.cmd_time = None;
            for t in 0..10 {
                far.change_time = Some(time(t));
                assert_eq!(far.get_need(time(100)), None);
            }
            // cmd_time only, no change time
            far.cmd_time = Some(time(0));
            far.change_time = None;
            assert_eq!(far.get_need(time(100)), immediate(action_fn()));
            // manually activate (tie!)
            far.change_time = Some(time(1));
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
            far.change_time = Some(time(0));
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
    // TODO: simplify all tests by using FetchAfterRule<T, S>
    #[test]
    fn fetch_after_volume_sets_change_time() {
        let mut fav = FetchAfterVolume::default();
        assert_eq!(fav.change_time, None);
        // notify [first] (t=0)
        fav.notify_playback(&PlaybackStatus::default());
        assert_eq!(fav.change_time, Some(time(0)));
        // notify [volume 0->50%] (t=1)
        fav.notify_playback(&PlaybackStatus {
            received_time: time(1),
            volume_percent: 50,
            ..PlaybackStatus::default()
        });
        assert_eq!(fav.change_time, Some(time(1)));
        // notify [identical] (t=1, still)
        fav.notify_playback(&PlaybackStatus {
            received_time: time(3),
            volume_percent: 50,
            ..PlaybackStatus::default()
        });
        assert_eq!(fav.change_time, Some(time(1)));
        // notify [volume 50->100%] (t=5)
        fav.notify_playback(&PlaybackStatus {
            received_time: time(5),
            volume_percent: 100,
            ..PlaybackStatus::default()
        });
        assert_eq!(fav.change_time, Some(time(5)));
    }
    #[test]
    fn fetch_after_volume_captures_cmd() {
        let mut fav = FetchAfterVolume::default();
        // default None
        assert_eq!(fav.cmd_time, None);
        // volume commands
        fav.notify_command(time(1), &Command::Volume { percent: 20 });
        assert_eq!(fav.cmd_time, Some(time(1)));
        fav.notify_command(time(2), &Command::VolumeRelative { percent_delta: -30 });
        assert_eq!(fav.cmd_time, Some(time(2)));
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
        for ignored_cmd in ignored_cmds {
            fav.notify_command(time(3), ignored_cmd); // ignore-cmd at t=3
            assert_eq!(fav.cmd_time, Some(time(2))); // unchanged (t=2)
        }
    }
    #[test]
    fn fetch_after_volume_gets_need() {
        let mut fav = FetchAfterVolume::default();
        // default -> None;
        assert_eq!(fav.get_need(time(0)), None);
        fav.change_time = None;
        fav.cmd_time = None;
        assert_eq!(fav.get_need(time(0)), None);
        // no cmd_time -> None
        fav.cmd_time = None;
        for t in 0..10 {
            fav.change_time = Some(time(t));
            assert_eq!(fav.get_need(time(100)), None);
        }
        // cmd_time only, no change time
        fav.cmd_time = Some(time(0));
        fav.change_time = None;
        assert_eq!(
            fav.get_need(time(100)),
            immediate(Action::fetch_playback_status())
        );
        // manually activate (tie!)
        fav.change_time = Some(time(1));
        fav.cmd_time = Some(time(1));
        assert_eq!(
            fav.get_need(time(2)),
            immediate(Action::fetch_playback_status())
        );
        assert_eq!(
            fav.get_need(time(1)),
            some_millis(
                FetchAfterVolume::DELAY_MILLIS,
                Action::fetch_playback_status()
            )
        );
        assert_eq!(
            fav.get_need(time(0)),
            some_millis(
                1000 + FetchAfterVolume::DELAY_MILLIS,
                Action::fetch_playback_status()
            )
        );
    }
}
