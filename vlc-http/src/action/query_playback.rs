// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use super::{
    response, ClientState, Endpoint, Error, Poll, Pollable, PollableConstructor, Sequence,
};
use crate::client_state::ClientStateSequence;

/// Query the playback status
#[derive(Clone, Debug)]
pub struct QueryPlayback {
    start_sequence: Sequence,
}
impl Pollable for QueryPlayback {
    type Output<'a> = &'a response::PlaybackStatus;

    fn next<'a>(&mut self, state: &'a ClientState) -> Result<Poll<Self::Output<'a>>, Error> {
        let playback_status = state.playback_status();
        let status_updated = playback_status
            .get_sequence()
            .is_after(self.start_sequence)?;
        let poll = match &**playback_status {
            Some(playback) if status_updated => Poll::Done(playback),
            _ => Poll::Need(Endpoint::query_status()),
        };
        Ok(poll)
    }
}
impl PollableConstructor for QueryPlayback {
    type Args = ();

    fn new((): Self::Args, state: ClientStateSequence) -> Self {
        let start_sequence = state.playback_status();
        Self { start_sequence }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::{Action, Response};
    use std::str::FromStr as _;

    const RESPONSE_STATUS_SIMPLE: &str = r#"{
      "rate":1,
      "time":456,
      "repeat":false,
      "loop":true,
      "length":910,
      "random":true,
      "apiversion":3,
      "seek_sec":10,
      "version":"3.0.20 Vetinari",
      "currentplid":438,
      "position":0.11884185671806,
      "volume":269,
      "state":"playing",
      "information":{
        "category":{
          "meta":{
            "artist":"Jimmy Fontanez",
            "album":"Royalty Free Music",
            "track_total":"0",
            "title":"Floaters",
            "track_number":"0"
          }
        }
      }
    }"#;

    #[test]
    fn caches() {
        let mut state = ClientState::new();

        let mut query1 = Action::query_playback(state.get_ref());
        let mut query2 = Action::query_playback(state.get_ref());

        // both request `status.json`
        insta::assert_ron_snapshot!(query1.next(&state).unwrap(), @r###"
        Need(Endpoint(
          path_and_query: "/requests/status.json",
        ))
        "###);
        insta::assert_ron_snapshot!(query2.next(&state).unwrap(), @r###"
        Need(Endpoint(
          path_and_query: "/requests/status.json",
        ))
        "###);

        // single `status.json` response
        state.update(Response::from_str(RESPONSE_STATUS_SIMPLE).expect("valid response"));

        // both resolve
        insta::assert_ron_snapshot!(query1.next(&state).unwrap(), @r###"
        Done(Status(
          apiversion: 3,
          information: Some(Info(
            title: "Floaters",
            artist: "Jimmy Fontanez",
            album: "Royalty Free Music",
            date: "",
            track_number: "0",
            track_total: "0",
            extra: {},
            playlist_item_id: Some(438),
          )),
          is_loop_all: true,
          is_random: true,
          is_repeat_one: false,
          version: "3.0.20 Vetinari",
          volume_percent: 105,
          mode: Playing,
          duration_secs: 910,
          position_secs: 456,
          position_fraction: 0.11884185671806,
          rate_ratio: 1.0,
        ))
        "###);
        insta::assert_ron_snapshot!(query2.next(&state).unwrap(), @r###"
        Done(Status(
          apiversion: 3,
          information: Some(Info(
            title: "Floaters",
            artist: "Jimmy Fontanez",
            album: "Royalty Free Music",
            date: "",
            track_number: "0",
            track_total: "0",
            extra: {},
            playlist_item_id: Some(438),
          )),
          is_loop_all: true,
          is_random: true,
          is_repeat_one: false,
          version: "3.0.20 Vetinari",
          volume_percent: 105,
          mode: Playing,
          duration_secs: 910,
          position_secs: 456,
          position_fraction: 0.11884185671806,
          rate_ratio: 1.0,
        ))
        "###);
    }

    #[test]
    fn accept_no_op() {
        let mut state = ClientState::new();

        // initialize state before creating query
        state.update(Response::from_str(RESPONSE_STATUS_SIMPLE).expect("valid response"));

        let mut query = Action::query_playback(state.get_ref());

        insta::assert_ron_snapshot!(query.next(&state).unwrap(), @r###"
        Need(Endpoint(
          path_and_query: "/requests/status.json",
        ))
        "###);

        // replay same response
        state.update(Response::from_str(RESPONSE_STATUS_SIMPLE).expect("valid response"));

        // still resolves (don't wait for a change!)
        insta::assert_ron_snapshot!(query.next(&state).unwrap(), @r###"
        Done(Status(
          apiversion: 3,
          information: Some(Info(
            title: "Floaters",
            artist: "Jimmy Fontanez",
            album: "Royalty Free Music",
            date: "",
            track_number: "0",
            track_total: "0",
            extra: {},
            playlist_item_id: Some(438),
          )),
          is_loop_all: true,
          is_random: true,
          is_repeat_one: false,
          version: "3.0.20 Vetinari",
          volume_percent: 105,
          mode: Playing,
          duration_secs: 910,
          position_secs: 456,
          position_fraction: 0.11884185671806,
          rate_ratio: 1.0,
        ))
        "###);
    }
}
