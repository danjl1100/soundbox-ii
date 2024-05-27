// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use vlc_http::{ClientState, Pollable as _};

#[derive(Clone, Default, serde::Serialize)]
pub struct Model {
    #[serde(skip)]
    items_created: u32,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    items: Vec<Item>,
    // TODO
    // playing: bool,
    #[serde(skip_serializing_if = "bool_is_false")]
    is_loop_all: bool,
    #[serde(skip_serializing_if = "bool_is_false")]
    is_repeat_one: bool,
    #[serde(skip_serializing_if = "bool_is_false")]
    is_random: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    unknown_endpoints: Vec<String>,
}
#[derive(Clone, serde::Serialize)]
pub(crate) struct Item {
    id: u32,
    uri: String,
}
impl Model {
    pub fn request(&mut self, endpoint: &str) -> String {
        let dummy_state = ClientState::new();
        let playlist = vlc_http::Action::query_playlist(&dummy_state)
            .next_endpoint(&dummy_state)
            .expect("dummy playlist path");
        let playlist = playlist.get_path_and_query();
        let playback = vlc_http::Action::query_playback(&dummy_state)
            .next_endpoint(&dummy_state)
            .expect("dummy playback path");
        let playback = playback.get_path_and_query();

        // FIXME improve parsing strategy
        let (path, args) =
            endpoint
                .split_once('?')
                .map_or((endpoint, Vec::new()), |(base, args)| {
                    (
                        base,
                        args.split('&')
                            .map(|arg| {
                                arg.split_once('=')
                                    .map_or((arg, None), |(key, val)| (key, Some(val)))
                            })
                            .collect(),
                    )
                });

        let command = args
            .iter()
            .find_map(|&(key, val)| (key == "command").then_some(val).flatten());
        let args: Vec<_> = args
            .into_iter()
            .filter(|&(key, _val)| key != "command")
            .collect();

        let response = if path == playlist {
            match command {
                Some("in_enqueue") => self.enqueue(&args),
                Some("pl_delete") => self.delete(&args),
                Some(_) => None, // unknown
                None => todo!(), // TODO: self.get_playlist_info(),
            }
        } else if path == playback {
            if args.is_empty() {
                match command {
                    None => Some(self.get_playback_status()),
                    Some("pl_random") => Some(self.toggle_random()),
                    Some("pl_loop") => Some(self.toggle_loop_all()),
                    Some("pl_repeat") => Some(self.toggle_repeat_one()),
                    Some(_) => todo!("command {command:?} (no args)"),
                }
            } else {
                match command {
                    None => None, // unknown (non-empty) args
                    Some(_) => todo!("command {command:?}, args {args:?}"),
                }
            }
        } else {
            None
        };

        if let Some(response) = response {
            response
        } else {
            self.unknown_endpoints.push(endpoint.to_owned());
            self.get_playlist_info()
        }
    }
    fn enqueue(&mut self, args: &[(&str, Option<&str>)]) -> Option<String> {
        let [("input", Some(val))] = *args else {
            return None;
        };

        let id = self.items_created;
        self.items_created += 1;
        let uri = val.to_owned();

        self.items.push(Item { id, uri });

        Some(self.get_playlist_info())
    }
    fn delete(&mut self, args: &[(&str, Option<&str>)]) -> Option<String> {
        let [("id", Some(val))] = *args else {
            return None;
        };

        let id: u32 = val.parse::<u32>().ok()?;

        self.items.retain(|item| item.id != id);

        Some(self.get_playlist_info())
    }

    fn toggle_random(&mut self) -> String {
        self.is_random = !self.is_random;
        self.get_playback_status()
    }
    fn toggle_loop_all(&mut self) -> String {
        self.is_loop_all = !self.is_loop_all;
        self.get_playback_status()
    }
    fn toggle_repeat_one(&mut self) -> String {
        self.is_repeat_one = !self.is_repeat_one;
        self.get_playback_status()
    }

    fn get_playlist_info(&self) -> String {
        let items = self
            .items
            .iter()
            .map(|Item { id, uri }| {
                serde_json::json!({
                    // arbitrary (deterministic)
                    "duration": id*100+(7*(id % 3)),
                    "uri": uri,
                    "type": "leaf",
                    "id": id.to_string(),
                    "ro": "rw",
                    "name": format!("Item {id}"),
                })
            })
            .collect::<Vec<_>>();
        serde_json::json!({
            "children":[{
                "children": items,
                "name":"Playlist",
            }]
        })
        .to_string()
    }

    fn get_playback_status(&self) -> String {
        serde_json::json!({
            "rate":1,
            "time":0,
            "repeat": self.is_repeat_one,
            "loop": self.is_loop_all,
            "length":0,
            "random": self.is_random,
            "apiversion":3,
            "version":"3.0.20 Vetinari",
            "currentplid":438,
            "position":0.0,
            "volume":256,
            "state":"playing",
            "information":{"category":{"meta":{}}},
        })
        .to_string()
    }
}

#[allow(clippy::trivially_copy_pass_by_ref)] // signature required by serde
fn bool_is_false(value: &bool) -> bool {
    !(*value)
}