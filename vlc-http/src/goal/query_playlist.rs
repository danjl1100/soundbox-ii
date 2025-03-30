// Copyright (C) 2021-2025  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use super::{response, ClientState, Endpoint, Error, Plan, PlanConstructor, Sequence, Step};
use crate::client_state::ClientStateSequence;

/// Query the playlist items
#[derive(Clone, Debug)]
#[must_use]
pub struct QueryPlaylist {
    start_sequence: Sequence,
}
impl Plan for QueryPlaylist {
    type Output<'a> = &'a [response::playlist::Item];

    fn next<'a>(&mut self, state: &'a ClientState) -> Result<Step<Self::Output<'a>>, Error> {
        let playlist_info = state.playlist_info();
        let step = if playlist_info.get_sequence().is_after(self.start_sequence)? {
            Step::Done(&playlist_info.items[..])
        } else {
            Step::Need(Endpoint::query_playlist())
        };
        Ok(step)
    }
}
impl PlanConstructor for QueryPlaylist {
    type Args = ();
    fn new((): Self::Args, state: ClientStateSequence) -> Self {
        let start_sequence = state.playlist_info();
        Self { start_sequence }
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::Response;
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

        let mut query1 = state.build_plan().query_playlist();
        let mut query2 = state.build_plan().query_playlist();

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

        let mut query = state.build_plan().query_playlist();

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
