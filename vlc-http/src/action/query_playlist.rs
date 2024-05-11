// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use super::{response, ClientState, Endpoint, Pollable, PollableConstructor, Sequence};

#[derive(Clone, Debug)]
/// Query the playlist items
pub struct QueryPlaylist {
    start_sequence: Sequence,
}
impl Pollable for QueryPlaylist {
    type Output = Vec<response::playlist::Item>;

    fn next_endpoint(&mut self, state: &ClientState) -> Result<Endpoint, Self::Output> {
        let playlist_info = state.playlist_info();
        if playlist_info.get_sequence() > self.start_sequence {
            Err(playlist_info.items.clone())
        } else {
            Ok(Endpoint::query_playlist())
        }
    }
}
impl PollableConstructor for QueryPlaylist {
    type Args = ();
    fn new((): Self::Args, state: &ClientState) -> Self {
        let start_sequence = state.playlist_info().get_sequence();
        Self { start_sequence }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Action;
    use std::str::FromStr;

    #[test]
    fn caches() {
        let mut state = ClientState::default();

        let mut query1 = Action::query_playlist(&state);
        let mut query2 = Action::query_playlist(&state);

        // both request `playlist.json`
        insta::assert_ron_snapshot!(query1.next_endpoint(&state), @r###"
        Ok(Endpoint(
          path_and_query: "/requests/playlist.json",
        ))
        "###);
        insta::assert_ron_snapshot!(query2.next_endpoint(&state), @r###"
        Ok(Endpoint(
          path_and_query: "/requests/playlist.json",
        ))
        "###);

        // single `playlist.json` response
        state.update(
            crate::Response::from_str(
                r#"{"children":[{"children":[
                    {
                      "duration":4567,
                      "uri":"file:///path/to/Music/Jimmy Fontanez/Floaters.mp3",
                      "type":"leaf",
                      "id":"123",
                      "ro":"rw",
                      "name":"Floaters.mp3"
                    }
                ],"name":"Playlist"}]}"#,
            )
            .expect("valid response"),
        );

        // both resolve
        insta::assert_ron_snapshot!(query1.next_endpoint(&state), @r###"
        Err([
          Item(
            duration_secs: Some(4567),
            id: "123",
            name: "Floaters.mp3",
            url: "file:///path/to/Music/Jimmy%20Fontanez/Floaters.mp3",
          ),
        ])
        "###);
        insta::assert_ron_snapshot!(query2.next_endpoint(&state), @r###"
        Err([
          Item(
            duration_secs: Some(4567),
            id: "123",
            name: "Floaters.mp3",
            url: "file:///path/to/Music/Jimmy%20Fontanez/Floaters.mp3",
          ),
        ])
        "###);
    }
}
