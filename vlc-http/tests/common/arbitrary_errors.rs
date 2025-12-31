// Copyright (C) 2021-2025  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Injects imperfections between the `vlc-http` logic and the simulated VLC instance

use self::alphanum_string::AlphanumString;
use self::arb_mode::ArbRepeatMode;
use self::ascii_string::AsciiString;
use self::model_endpoint_caller::ModelEndpointCaller;

use std::str::FromStr as _;
use url::Url;
use vlc_http::{Change, ClientState, goal::TargetPlaylistItems};

#[derive(arbitrary::Arbitrary)]
struct ArbChangesList {
    changes: Vec<ArbChange>,
}
#[derive(arbitrary::Arbitrary)]
enum ArbChange {
    PlaybackMode {
        repeat: ArbRepeatMode,
        is_random: bool,
    },
    PlaylistSet {
        items: Vec<
            AlphanumString, // AsciiString
        >,
    },
}

impl ArbChange {
    fn get_complexity(&self, current_len: usize) -> usize {
        match self {
            ArbChange::PlaybackMode {
                repeat: _,
                is_random: _,
            } => 4,
            ArbChange::PlaylistSet { items } => 2 * items.len() + current_len + 4,
        }
    }
}

mod ascii_string {
    #[derive(Debug)]
    pub struct AsciiString(String);
    impl<'a> arbitrary::Arbitrary<'a> for AsciiString {
        fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
            let string: Result<_, _> = u
                .arbitrary_iter()?
                .map(|byte: Result<u8, _>| {
                    let ascii_byte = byte? & 0x7F;
                    Ok(ascii_byte as char)
                })
                .collect();
            string.map(Self)
        }
    }
    impl std::ops::Deref for AsciiString {
        type Target = String;
        fn deref(&self) -> &Self::Target {
            let Self(inner) = self;
            inner
        }
    }
    impl std::fmt::Display for AsciiString {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let Self(inner) = self;
            write!(f, "{inner}")
        }
    }
}
mod alphanum_string {
    #[derive(Debug)]
    pub struct AlphanumString(String);
    impl<'a> arbitrary::Arbitrary<'a> for AlphanumString {
        fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
            let string: Result<_, _> = u
                .arbitrary_iter()?
                .map(|byte: Result<u8, _>| {
                    let ascii_byte = match byte? & 63 {
                        v @ 0..26 => b'a' + v,
                        v @ 26..52 => b'A' + (v - 26),
                        v @ 52..62 => b'0' + (v - 52),
                        62 => b'_',
                        63 => b'.',
                        _ => unreachable!(),
                    };
                    Ok(ascii_byte as char)
                })
                .collect();
            string.map(Self)
        }
    }
    impl std::ops::Deref for AlphanumString {
        type Target = String;
        fn deref(&self) -> &Self::Target {
            let Self(inner) = self;
            inner
        }
    }
    impl std::fmt::Display for AlphanumString {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let Self(inner) = self;
            write!(f, "{inner}")
        }
    }
}

mod arb_mode {
    use vlc_http::goal::RepeatMode;

    #[derive(arbitrary::Arbitrary)]
    pub enum ArbRepeatMode {
        Off,
        All,
        One,
    }
    impl From<RepeatMode> for ArbRepeatMode {
        fn from(value: RepeatMode) -> Self {
            match value {
                RepeatMode::Off => Self::Off,
                RepeatMode::All => Self::All,
                RepeatMode::One => Self::One,
            }
        }
    }
    impl From<ArbRepeatMode> for RepeatMode {
        fn from(value: ArbRepeatMode) -> Self {
            match value {
                ArbRepeatMode::Off => Self::Off,
                ArbRepeatMode::All => Self::All,
                ArbRepeatMode::One => Self::One,
            }
        }
    }
}

impl From<ArbChange> for Change {
    fn from(value: ArbChange) -> Self {
        match value {
            ArbChange::PlaybackMode { repeat, is_random } => vlc_http::goal::PlaybackMode::new()
                .set_repeat(repeat.into())
                .set_random(is_random)
                .into(),
            ArbChange::PlaylistSet { items } => {
                let items = items
                    .into_iter()
                    .map(|s| Url::from_str(&format!("file:///{s}")).expect("valid URL"))
                    .collect();
                TargetPlaylistItems::new().set_urls(items).into()
            }
        }
    }
}

mod model_endpoint_caller {
    use std::str::FromStr as _;
    use vlc_http::{
        Endpoint,
        sync::EndpointRequestor,
        testing::{Model, ModelResponse},
    };

    pub struct ModelEndpointCaller(Model);
    impl ModelEndpointCaller {
        pub fn new() -> Self {
            Self(Model::default())
        }
        pub fn get_model(&self) -> &Model {
            let Self(model) = self;
            model
        }
    }
    impl EndpointRequestor for ModelEndpointCaller {
        type Error = Error;

        fn request(&mut self, endpoint: Endpoint) -> Result<vlc_http::Response, Self::Error> {
            let Self(model) = self;

            let make_err = |kind| Error { kind };

            let response = model
                .request(endpoint.get_path_and_query())
                .map_err(|source| ErrorKind::Request { source, endpoint })
                .map_err(make_err)?;
            let response = match &response {
                vlc_http::testing::ModelResponse::Json(s) => vlc_http::Response::from_str(s)
                    .map_err(|source| ErrorKind::ResponseJson { source, response })
                    .map_err(make_err)?,
                vlc_http::testing::ModelResponse::Art => {
                    return Err(make_err(ErrorKind::ResponseArt));
                }
            };
            Ok(response)
        }
    }

    #[derive(Debug)]
    pub struct Error {
        kind: ErrorKind,
    }
    #[derive(Debug)]
    enum ErrorKind {
        Request {
            source: vlc_http::testing::RequestError,
            endpoint: Endpoint,
        },
        ResponseJson {
            source: vlc_http::response::ParseError,
            response: ModelResponse,
        },
        ResponseArt,
    }
    impl std::error::Error for Error {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            let Self { kind } = self;
            match kind {
                ErrorKind::Request { source, .. } => Some(source),
                ErrorKind::ResponseJson { source, .. } => Some(source),
                ErrorKind::ResponseArt => None,
            }
        }
    }
    impl std::fmt::Display for Error {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let Self { kind } = self;
            match kind {
                ErrorKind::Request {
                    source: _,
                    endpoint,
                } => write!(f, "model rejected request for {endpoint:?}"),
                ErrorKind::ResponseJson {
                    source: _,
                    response,
                } => {
                    write!(f, "invalid JSON response from model: {response:?}")
                }
                ErrorKind::ResponseArt => todo!(),
            }
        }
    }
}

#[test]
fn arb_commands_perfect() {
    arbtest::arbtest(|u| {
        let mut client_state = ClientState::new();

        let mut endpoint_caller = ModelEndpointCaller::new();

        let ArbChangesList { changes } = u.arbitrary()?;
        for change in changes {
            let current_len = endpoint_caller.get_model().get_items().len();
            let complexity = change.get_complexity(current_len);

            let change = change.into();
            dbg!((&change, complexity));
            let plan = client_state.build_plan().apply(change);

            let max_iter_count = complexity;
            vlc_http::sync::complete_plan(
                plan,
                &mut client_state,
                &mut endpoint_caller,
                max_iter_count,
            )
            .expect("complete plan success");
        }

        Ok(())
    });
}
