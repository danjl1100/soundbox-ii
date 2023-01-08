// Copyright (C) 2021-2023  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use crate::vlc_responses::{PlaybackStatus, PlaybackTiming, PlaylistInfo};
use crate::{Action, LowCommand as Command};
use shared::Time;

use need::ord as ord_need;
use need::Need;
mod need;

use fill::{FillPlayback, FillPlaylist};
mod fill;

use fetch_after::{FetchAfterRule, FetchAfterSpec};
mod fetch_after;

use fetch_after_seek::FetchAfterSeek;
mod fetch_after_seek;

use fetch_after_volume::FetchAfterVolume;
mod fetch_after_volume;

use fetch_after_track_end::FetchAfterTrackEnd;
mod fetch_after_track_end;

trait Rule: Send + Sync + core::fmt::Debug {
    fn get_need(&self, now: Time) -> Need;
    fn notify_playback(&mut self, _playback: &PlaybackStatus) {}
    fn notify_playlist(&mut self, _playlist: &PlaylistInfo) {}
    fn notify_command(&mut self, _now: Time, _command: &Command) {}
}

pub(crate) struct Rules {
    rules: Vec<Box<dyn Rule>>,
}
impl Rules {
    pub fn new() -> Self {
        Self {
            rules: vec![
                Box::<FillPlayback>::default(),
                Box::<FillPlaylist>::default(),
                Box::new(FetchAfterRule::from_spec(FetchAfterSeek)),
                Box::new(FetchAfterRule::from_spec(FetchAfterVolume)),
                Box::<FetchAfterTrackEnd>::default(),
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

#[cfg(test)]
mod tests {
    pub(super) use super::need::tests::{immediate, some_millis};
    pub(super) use crate::vlc_responses::{PlaybackInfo, PlaylistItem};
    pub(super) fn time(secs: i64) -> shared::Time {
        shared::time_from_secs_opt(secs).expect("valid seconds input in test")
    }
}
