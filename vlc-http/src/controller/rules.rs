use crate::{Action, Command, PlaybackStatus, PlaylistInfo};
use shared::{Time, TimeDifference};
use std::cmp::Ordering;
use std::time::Duration;
use tokio::sync::watch;

type Need = Option<(Option<Duration>, Action)>;
fn ord_need(lhs: &Need, rhs: &Need) -> Ordering {
    use Ordering::{Equal, Greater, Less};
    match (lhs, rhs) {
        (None, None) => Equal,
        (Some(_), None) => Less,
        (None, Some(_)) => Greater,
        (Some(lhs), Some(rhs)) => match (lhs, rhs) {
            ((None, _), (None, _)) => Equal,
            ((Some(_), _), (None, _)) => Less,
            ((None, _), (Some(_), _)) => Greater,
            ((Some(lhs), _), (Some(rhs), _)) => lhs.cmp(rhs),
        },
    }
}

type Playback = Option<PlaybackStatus>;
type Playlist = Option<PlaylistInfo>;

pub(crate) struct Rules {
    playback_status: watch::Receiver<Playback>,
    playlist_info: watch::Receiver<Playlist>,
    rules: Vec<Box<dyn Rule>>,
}
impl Rules {
    pub fn new(
        playback_status: watch::Receiver<Playback>,
        playlist_info: watch::Receiver<Playlist>,
    ) -> Self {
        Self {
            playback_status,
            playlist_info,
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
        //  (1) calculate all `Need`s (TODO: can short-circuit on first no-delay item)
        let needs = {
            let playback = self.playback_status.borrow().clone(); //TODO: remove this expensive clone?
            let playlist = self.playlist_info.borrow().clone();
            self.rules.iter_mut().map(move |rule| {
                rule.notify_info(&playback, &playlist);
                rule.get_need(now)
            })
        };
        //  (2) pick most-immediate option
        needs.min_by(ord_need).flatten()
    }
    pub fn notify_command(&mut self, now: Time, cmd: &Command) {
        for rule in &mut self.rules {
            rule.notify_command(now, cmd);
        }
    }
}

trait Rule: Send + Sync {
    fn notify_info(&mut self, playback: &Playback, playlist: &Playlist);
    fn get_need(&self, now: Time) -> Need;
    fn notify_command(&mut self, _now: Time, _command: &Command) {}
}

#[derive(Default)]
struct FillPlayback(Option<()>);
impl Rule for FillPlayback {
    fn notify_info(&mut self, playback: &Playback, _: &Playlist) {
        self.0 = playback.as_ref().map(|_| ());
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
    fn notify_info(&mut self, _: &Playback, playlist: &Playlist) {
        self.0 = playlist.as_ref().map(|_| ());
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
    fn notify_info(&mut self, playback: &Playback, _: &Playlist) {
        let item_time_and_info = playback.as_ref().map(|playback| {
            let received_time = playback.received_time;
            let duration = playback.duration;
            let item_id = playback
                .information
                .as_ref()
                .and_then(|info| info.playlist_item_id);
            (received_time, (duration, item_id))
        });
        self.change_time = match (item_time_and_info, self.item_info.take()) {
            (Some((item_time, item_info)), Some(prev_item_info)) => {
                if item_info == prev_item_info {
                    println!("IDENTICAL: {:?} ===> {:?}", prev_item_info, item_info);
                    self.change_time
                } else {
                    println!("CHANGED: {:?} ===> {:?}", prev_item_info, item_info);
                    Some(item_time)
                }
            }
            (None, _) => None,
            (Some((item_change_time, _)), None) => Some(item_change_time),
        };
        self.item_info = item_time_and_info.map(|(_time, info)| info);
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

    #[test]
    fn fills_playback() {
        let dummy_time = time(0);
        let uut = &mut FillPlayback::default() as &mut dyn Rule;
        // initial -> fetch
        assert_eq!(
            uut.get_need(dummy_time),
            immediate(Action::fetch_playback_status())
        );
        // cleared -> fetch
        uut.notify_info(&None, &None);
        assert_eq!(
            uut.get_need(dummy_time),
            immediate(Action::fetch_playback_status())
        );
        // set -> no action
        let playback = PlaybackStatus::default();
        uut.notify_info(&Some(playback), &None);
        assert_eq!(uut.get_need(dummy_time), None);
        // cleared -> fetch
        uut.notify_info(&None, &None);
        assert_eq!(
            uut.get_need(dummy_time),
            immediate(Action::fetch_playback_status())
        );
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
        // cleared -> fetch
        uut.notify_info(&None, &None);
        assert_eq!(
            uut.get_need(dummy_time),
            immediate(Action::fetch_playlist_info())
        );
        // set -> no action
        let playlist = PlaylistInfo::default();
        uut.notify_info(&None, &Some(playlist));
        assert_eq!(uut.get_need(dummy_time), None);
        // cleared -> fetch
        uut.notify_info(&None, &None);
        assert_eq!(
            uut.get_need(dummy_time),
            immediate(Action::fetch_playlist_info())
        );
    }
    fn assert_none_initial_cleared(uut: &mut dyn Rule) {
        let time_0 = time(0);
        // initial state -> no output
        assert_eq!(uut.get_need(time_0), None);
        // cleared state -> no output
        uut.notify_info(&None, &None);
        assert_eq!(uut.get_need(time_0), None);
    }
    #[test]
    fn fetch_after_seek_sets_change_time() {
        let mut fas = FetchAfterSeek::default();
        assert_eq!(fas.change_time, None);
        // notify [first] (t=0)
        fas.notify_info(&Some(PlaybackStatus::default()), &None);
        assert_eq!(fas.change_time, Some(time(0)));
        // notify [duration 0->1] (t=1)
        fas.notify_info(
            &Some(PlaybackStatus {
                received_time: time(1),
                duration: 2,
                ..PlaybackStatus::default()
            }),
            &None,
        );
        assert_eq!(fas.change_time, Some(time(1)));
        // notify [identical] (t=1, still)
        fas.notify_info(
            &Some(PlaybackStatus {
                received_time: time(3),
                duration: 2,
                ..PlaybackStatus::default()
            }),
            &None,
        );
        assert_eq!(fas.change_time, Some(time(1)));
        // notify [info None->Some(id=None)] (t=1, still)
        fas.notify_info(
            &Some(PlaybackStatus {
                received_time: time(4),
                duration: 2,
                information: Some(PlaybackInfo::default()),
                ..PlaybackStatus::default()
            }),
            &None,
        );
        assert_eq!(fas.change_time, Some(time(1)));
        // notify [id None -> Some(10)] (t=5)
        fas.notify_info(
            &Some(PlaybackStatus {
                received_time: time(5),
                duration: 2,
                information: Some(PlaybackInfo {
                    playlist_item_id: Some(10),
                    ..PlaybackInfo::default()
                }),
                ..PlaybackStatus::default()
            }),
            &None,
        );
        assert_eq!(fas.change_time, Some(time(5)));
        // notify [id Some(10) -> Some(22)] (t=6)
        fas.notify_info(
            &Some(PlaybackStatus {
                received_time: time(6),
                duration: 2,
                information: Some(PlaybackInfo {
                    playlist_item_id: Some(22),
                    ..PlaybackInfo::default()
                }),
                ..PlaybackStatus::default()
            }),
            &None,
        );
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
