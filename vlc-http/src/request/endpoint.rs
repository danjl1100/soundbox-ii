// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! HTTP-level primitives (interchange for test purposes)

use crate::{
    command::{VolumePercent256, VolumePercentDelta256},
    Command,
};

/// VLC HTTP endpoint information to execute a [`Command`]
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[must_use]
pub struct Endpoint {
    path_and_query: String,
}
impl Endpoint {
    /// Returns the combined HTTP path and query string for the endpoint
    #[must_use]
    pub fn path_and_query(&self) -> &str {
        &self.path_and_query
    }
    /// Returns the HTTP method for the endpoint
    #[must_use]
    pub fn method(&self) -> http::Method {
        // NOTE: this is a function for future expansion purposes
        http::Method::GET
    }
}

mod endpoint_args {
    use super::Endpoint;
    use std::borrow::Cow;
    use std::fmt::Write;

    /// Builder for [`Endpoint`]
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub(crate) struct EndpointArgs {
        // TODO combine path, query into one field (e.g. if query grows large, reduce amount of
        // buffer copying in `finish`
        path: &'static str,
        query: Cow<'static, str>,
        // command: &'static str,
        // args: Vec<(&'static str, String)>,
    }
    impl EndpointArgs {
        fn new(path: &'static str, command: Option<&'static str>) -> Self {
            let mut this = Self {
                path,
                query: "".into(),
            };
            if let Some(command) = command {
                this = this.append("command", command);
            }
            this
        }
        pub fn new_status(command: &'static str) -> Self {
            const PATH_STATUS_JSON: &str = "/requests/status.json";
            Self::new(PATH_STATUS_JSON, Some(command))
        }
        pub fn new_playlist(command: &'static str) -> Self {
            const PATH_PLAYLIST_JSON: &str = "/requests/playlist.json";
            Self::new(PATH_PLAYLIST_JSON, Some(command))
        }
        pub fn new_art(id: &str) -> Self {
            const PATH_ART: &str = "/art";
            Self::new(PATH_ART, None).append("item", id)
        }
        //
        pub fn append(self, key: &str, value: &str) -> Self {
            let key = urlencoding::encode(key);
            let value = urlencoding::encode(value);
            self.append_raw(&key, &value)
        }
        pub fn append_url(self, key: &str, value: &url::Url) -> Self {
            let key = urlencoding::encode(key);
            // `url::Url` already applies URL encoding,
            // and VLC does not understand a doubly-encoded URL
            let value = value.as_str();
            self.append_raw(&key, value)
        }
        fn append_raw(mut self, key: &str, value: &str) -> Self {
            let sep = if self.query.is_empty() { "" } else { "&" };
            write!(self.query.to_mut(), "{sep}{key}={value}").expect("string write succeeds");
            self
        }
        pub fn finish(self) -> Endpoint {
            let Self { path, query } = self;
            let path_and_query = format!("{path}?{query}");
            Endpoint { path_and_query }
        }
    }
}

impl Command {
    /// Creates a request for the current art
    pub fn art(id: &str) -> Endpoint {
        endpoint_args::EndpointArgs::new_art(id).finish()
    }
}
impl From<Command> for Endpoint {
    /// Creates a request for the specified command
    fn from(command: Command) -> Self {
        use endpoint_args::EndpointArgs as Args;
        match command {
            Command::PlaylistAdd { url } => {
                Args::new_playlist("in_enqueue").append_url("input", &url)
            }
            Command::PlaylistDelete { item_id } => {
                Args::new_playlist("pl_delete").append("id", &item_id.to_string())
            }
            Command::PlaylistPlay { item_id } => {
                let mut args = Args::new_status("pl_play");
                if let Some(item_id) = item_id {
                    args = args.append("id", &item_id);
                }
                args
            }
            Command::PlaybackResume => Args::new_status("pl_forceresume"),
            Command::PlaybackPause => Args::new_status("pl_forcepause"),
            Command::PlaybackStop => Args::new_status("pl_stop"),
            Command::SeekNext => Args::new_status("pl_next"),
            Command::SeekPrevious => Args::new_status("pl_previous"),
            Command::SeekTo { seconds } => {
                Args::new_status("seek").append("val", &seconds.to_string())
            }
            Command::SeekRelative { seconds_delta } => {
                Args::new_status("seek").append("val", &seconds_delta.to_string())
            }
            Command::Volume { percent } => Args::new_status("volume")
                .append("val", &VolumePercent256::from(percent).to_string()),
            Command::VolumeRelative { percent_delta } => Args::new_status("volume").append(
                "val",
                &VolumePercentDelta256::from(percent_delta).to_string(),
            ),
            Command::ToggleRandom => Args::new_status("pl_random"),
            Command::ToggleRepeatOne => Args::new_status("pl_repeat"),
            Command::ToggleLoopAll => Args::new_status("pl_loop"),
            Command::PlaybackSpeed { speed } => {
                Args::new_status("rate").append("val", &speed.to_string())
            }
        }
        .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playlist_play() {
        insta::assert_ron_snapshot!(Endpoint::from(Command::PlaylistPlay {
            item_id: None,
        }),
        @r###"
        Endpoint(
          path_and_query: "/requests/status.json?command=pl_play",
        )
        "###);
        insta::assert_ron_snapshot!(Endpoint::from(Command::PlaylistPlay {
            item_id: Some("123abc".to_owned()),
        }),
        @r###"
        Endpoint(
          path_and_query: "/requests/status.json?command=pl_play&id=123abc",
        )
        "###);
    }

    // TODO
    // fn assert_encode<T, U>(value: T, expected: U)
    // where
    //     TextIntent: From<T> + From<U>,
    // {
    //     assert_eq!(TextIntent::from(value), TextIntent::from(expected));
    // }
    // #[test]
    // fn execs_simple_cmds() {
    //     assert_encode(
    //         LowCommand::PlaylistPlay { item_id: None },
    //         StatusIntent(Some(CmdArgs {
    //             command: "pl_play",
    //             args: vec![],
    //         })),
    //     );
    //     assert_encode(
    //         LowCommand::PlaybackResume,
    //         StatusIntent(Some(CmdArgs {
    //             command: "pl_forceresume",
    //             args: vec![],
    //         })),
    //     );
    //     assert_encode(
    //         LowCommand::PlaybackPause,
    //         StatusIntent(Some(CmdArgs {
    //             command: "pl_forcepause",
    //             args: vec![],
    //         })),
    //     );
    //     assert_encode(
    //         LowCommand::PlaybackStop,
    //         StatusIntent(Some(CmdArgs {
    //             command: "pl_stop",
    //             args: vec![],
    //         })),
    //     );
    //     assert_encode(
    //         LowCommand::SeekNext,
    //         StatusIntent(Some(CmdArgs {
    //             command: "pl_next",
    //             args: vec![],
    //         })),
    //     );
    //     assert_encode(
    //         LowCommand::SeekPrevious,
    //         StatusIntent(Some(CmdArgs {
    //             command: "pl_previous",
    //             args: vec![],
    //         })),
    //     );
    //     assert_encode(
    //         LowCommand::SeekTo { seconds: 259 },
    //         StatusIntent(Some(CmdArgs {
    //             command: "seek",
    //             args: vec![("val", "259".to_string())],
    //         })),
    //     );
    //     assert_encode(
    //         LowCommand::SeekRelative { seconds_delta: 32 },
    //         StatusIntent(Some(CmdArgs {
    //             command: "seek",
    //             args: vec![("val", "+32".to_string())],
    //         })),
    //     );
    //     assert_encode(
    //         LowCommand::SeekRelative { seconds_delta: -57 },
    //         StatusIntent(Some(CmdArgs {
    //             command: "seek",
    //             args: vec![("val", "-57".to_string())],
    //         })),
    //     );
    //     assert_encode(
    //         LowCommand::SeekRelative { seconds_delta: 0 },
    //         StatusIntent(Some(CmdArgs {
    //             command: "seek",
    //             args: vec![("val", "+0".to_string())],
    //         })),
    //     );
    //     assert_encode(
    //         LowCommand::PlaybackSpeed { speed: 0.21 },
    //         StatusIntent(Some(CmdArgs {
    //             command: "rate",
    //             args: vec![("val", "0.21".to_string())],
    //         })),
    //     );
    // }
    // #[test]
    // fn exec_url_encoded() {
    //     let url = url::Url::parse("file:///SENTINEL_%20_URL_%20%5E%24").expect("valid url");
    //     assert_encode(
    //         LowCommand::PlaylistAdd { url },
    //         PlaylistIntent(Some(CmdArgs {
    //             command: "in_enqueue",
    //             args: vec![("input", "file:///SENTINEL_ _URL_ ^$".to_string())],
    //         })),
    //     );
    //     let id_str = String::from("some id");
    //     assert_encode(
    //         LowCommand::PlaylistDelete {
    //             item_id: id_str.clone(),
    //         },
    //         PlaylistIntent(Some(CmdArgs {
    //             command: "pl_delete",
    //             args: vec![("id", id_str.clone())],
    //         })),
    //     );
    //     assert_encode(
    //         LowCommand::PlaylistPlay {
    //             item_id: Some(id_str.clone()),
    //         },
    //         StatusIntent(Some(CmdArgs {
    //             command: "pl_play",
    //             args: vec![("id", id_str)],
    //         })),
    //     );
    // }
    // #[test]
    // fn exec_volume() {
    //     use std::convert::TryFrom;
    //     let percent_vals = [
    //         (100, 256),
    //         (0, 0),
    //         (20, 51),  // round: 51.2 --> 51
    //         (40, 102), // round: 102.4 --> 102
    //         (60, 154), // round: 153.6 --> 154
    //         (80, 205), // round: 204.8 --> 205
    //         (200, 512),
    //     ];
    //     for (percent, val) in &percent_vals {
    //         assert_eq!(encode_volume_val(*percent), *val);
    //         assert_eq!(decode_volume_to_percent(*val), *percent);
    //         assert_encode(
    //             LowCommand::Volume { percent: *percent },
    //             StatusIntent(Some(CmdArgs {
    //                 command: "volume",
    //                 args: vec![("val", format!("{val}"))],
    //             })),
    //         );
    //         let percent_signed = i16::try_from(*percent).expect("test values within range");
    //         let check_relative = |sign, percent_delta| {
    //             assert_encode(
    //                 LowCommand::VolumeRelative { percent_delta },
    //                 StatusIntent(Some(CmdArgs {
    //                     command: "volume",
    //                     args: vec![("val", format!("{sign}{val}"))],
    //                 })),
    //             );
    //         };
    //         if percent_signed == 0 {
    //             check_relative("+", percent_signed);
    //         } else {
    //             check_relative("+", percent_signed);
    //             check_relative("-", -percent_signed);
    //         }
    //     }
    // }
}
