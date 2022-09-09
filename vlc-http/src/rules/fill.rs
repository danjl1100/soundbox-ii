// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use super::{need::Need, Action, PlaybackStatus, PlaylistInfo, Rule, Time};

#[derive(Default, Debug)]
pub(super) struct FillPlayback(Option<()>);
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
pub(super) struct FillPlaylist(Option<()>);
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
#[cfg(test)]
mod tests {
    use super::super::tests::{immediate, time};
    use super::*;

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
}
