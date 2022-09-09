// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use super::{Action, Need, PlaybackStatus, PlaybackTiming, Rule, Time};

#[derive(Default, Debug)]
pub(super) struct FetchAfterTrackEnd {
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
    use super::super::tests::{some_millis, time};
    use super::*;

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
}
