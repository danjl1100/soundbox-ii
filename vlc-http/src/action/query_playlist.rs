// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use super::{
    response, ClientState, Endpoint, Error, Poll, Pollable, PollableConstructor, Sequence,
};
use crate::client_state::ClientStateSequence;

/// Query the playlist items
#[derive(Clone, Debug)]
pub struct QueryPlaylist {
    start_sequence: Sequence,
}
impl Pollable for QueryPlaylist {
    type Output<'a> = &'a [response::playlist::Item];

    fn next<'a>(&mut self, state: &'a ClientState) -> Result<Poll<Self::Output<'a>>, Error> {
        let playlist_info = state.playlist_info();
        let poll = if playlist_info.get_sequence().is_after(self.start_sequence)? {
            Poll::Done(&playlist_info.items[..])
        } else {
            Poll::Need(Endpoint::query_playlist())
        };
        Ok(poll)
    }
}
impl PollableConstructor for QueryPlaylist {
    type Args = ();
    fn new((): Self::Args, state: ClientStateSequence) -> Self {
        let start_sequence = state.playlist_info();
        Self { start_sequence }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::{Action, Response};
    use std::str::FromStr;
    use test_log::test;

    const RESPONSE_PLAYLIST_SIMPLE: &str = r#"{"children":[{"children":[
        {
          "duration":4567,
          "uri":"file:///path/to/Music/Jimmy Fontanez/Floaters.mp3",
          "type":"leaf",
          "id":"123",
          "ro":"rw",
          "name":"Floaters.mp3"
        }
    ],"name":"Playlist"}]}"#;

    #[test]
    fn caches() {
        let mut state = ClientState::default();

        let mut query1 = Action::query_playlist(state.get_ref());
        let mut query2 = Action::query_playlist(state.get_ref());

        // both request `playlist.json`
        insta::assert_ron_snapshot!(query1.next(&state).unwrap(), @r###"
        Need(Endpoint(
          path_and_query: "/requests/playlist.json",
        ))
        "###);
        insta::assert_ron_snapshot!(query2.next(&state).unwrap(), @r###"
        Need(Endpoint(
          path_and_query: "/requests/playlist.json",
        ))
        "###);

        // single `playlist.json` response
        state.update(Response::from_str(RESPONSE_PLAYLIST_SIMPLE).expect("valid response"));

        // both resolve
        insta::assert_ron_snapshot!(query1.next(&state).unwrap(), @r###"
        Done([
          Item(
            duration_secs: Some(4567),
            id: 123,
            name: "Floaters.mp3",
            url: "file:///path/to/Music/Jimmy%20Fontanez/Floaters.mp3",
          ),
        ])
        "###);
        insta::assert_ron_snapshot!(query2.next(&state).unwrap(), @r###"
        Done([
          Item(
            duration_secs: Some(4567),
            id: 123,
            name: "Floaters.mp3",
            url: "file:///path/to/Music/Jimmy%20Fontanez/Floaters.mp3",
          ),
        ])
        "###);
    }

    #[test]
    fn accept_no_op() {
        let mut state = ClientState::default();

        // initialize state before creating query
        state.update(Response::from_str(RESPONSE_PLAYLIST_SIMPLE).expect("valid response"));

        let mut query = Action::query_playlist(state.get_ref());

        insta::assert_ron_snapshot!(query.next(&state).unwrap(), @r###"
        Need(Endpoint(
          path_and_query: "/requests/playlist.json",
        ))
        "###);

        // replay same response
        state.update(Response::from_str(RESPONSE_PLAYLIST_SIMPLE).expect("valid response"));

        // still resolves (don't wait for a change!)
        insta::assert_ron_snapshot!(query.next(&state).unwrap(), @r###"
        Done([
          Item(
            duration_secs: Some(4567),
            id: 123,
            name: "Floaters.mp3",
            url: "file:///path/to/Music/Jimmy%20Fontanez/Floaters.mp3",
          ),
        ])
        "###);
    }
}
