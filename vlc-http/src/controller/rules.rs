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
            Some(prev_item_info) if prev_item_info == item_info => {
                println!("IDENTICAL: {:?} ===> {:?}", prev_item_info, item_info);
                self.change_time
            }
            prev => {
                println!("CHANGED: {:?} ===> {:?}", prev, item_info);
                Some(item_time)
            }
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

// TODO!  delay until end of song, then update status!  (minimum delay of 1.0 second)
// struct FetchAfterTrackEnd {
//     todo: (),
// }
// impl Rule for FetchAfterTrackEnd {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vlc_responses::PlaybackInfo;
    use shared::time_from_secs as time;

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
    fn fetch_after_seek_captures_seek() {
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
}
