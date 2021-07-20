//! Reads state and sends commands to HTTP interface of VLC

// TODO: only while building
#![allow(dead_code)]
// teach me
#![deny(clippy::pedantic)]
// no unsafe
#![forbid(unsafe_code)]
// no unwrap
#![deny(clippy::unwrap_used)]
// no panic
#![deny(clippy::panic)]
// docs!
#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

use std::convert::TryFrom;

/// Control commands for VLC
#[derive(Debug, Clone, Copy)]
#[allow(clippy::pub_enum_variant_names)]
pub enum Command {
    /// Force playback to resume
    PlaybackResume,
    /// Force playback to pause
    PlaybackPause,
    /// Force playback to pause
    PlaybackStop,
    // /// Seek to the next item
    // SeekNext,
    // /// Seek to the previous item
    // SeekPrevious,
    // /// Seek within the current item
    // SeekTo {
    //     /// Seconds within the current item
    //     seconds: u32,
    // },
    // /// Set the playback volume
    // Volume {
    //     /// Percentage for the volume (clamped at 300, which means 300% volume)
    //     percent: u16,
    // },
    // /// Set the item selection mode
    // /// TODO: deleteme in Phase 2
    // PlaybackMode {
    //     #[allow(missing_docs)]
    //     repeat: RepeatMode,
    //     /// Randomizes the VLC playback order when `true`
    //     random: bool,
    // },
}
/// Information queries for VLC
pub enum Query {
    /// Album Artwork for the current item
    Art,
}

/// Rule for selecting the next playback item in the VLC queue
///
/// TODO: deleteme in Phase 2
pub enum RepeatMode {
    /// Stop the VLC queue after playing all items
    Off,
    /// Repeat the VLC queue after playing all items
    All,
    /// Repeat only the current item
    One,
}

pub use web::Authority;
pub(crate) use web::{Context, RequestIntent};
/// HTTP-specific primitives (interchange for test purposes)
pub mod web {
    use super::Credentials;

    pub use awc::{
        http::{
            uri::{Authority, InvalidUri, PathAndQuery, Uri},
            Method,
        },
        Client,
    };
    /// HTTP Request information
    #[must_use]
    #[derive(Debug, PartialEq, Eq)]
    pub(crate) struct RequestInfo {
        pub path_and_query: PathAndQuery,
        pub method: Method,
    }
    /// Description of a pending request to be executed
    #[must_use]
    #[allow(missing_docs)]
    #[derive(Debug)]
    pub enum RequestIntent<'a, 'b> {
        Status {
            command: &'a str,
            arg_str: Option<&'b str>,
        },
        Art {
            id: Option<u32>,
        },
        Playlist,
    }
    impl<'a, 'b> RequestIntent<'a, 'b> {
        pub(crate) fn status(command: &'a str) -> Self {
            Self::Status {
                command,
                arg_str: None,
            }
        }
    }
    impl<'a, 'b> From<RequestIntent<'a, 'b>> for RequestInfo {
        fn from(intent: RequestIntent<'a, 'b>) -> Self {
            const STATUS_JSON: &str = "/requests/status.json";
            const PLAYLIST_JSON: &str = "/requests/playlist.json";
            const ART: &str = "/art";
            let path_and_query = match intent {
                RequestIntent::Status {
                    command,
                    arg_str: None,
                } => format!("{}?command={}", STATUS_JSON, command)
                    .parse()
                    .expect("valid command in RequestIntent"),
                RequestIntent::Status {
                    command,
                    arg_str: Some(arg_str),
                } => format!("{}?command={}&{}", STATUS_JSON, command, arg_str)
                    .parse()
                    .expect("valid command and arg_str in RequestIntent"),
                RequestIntent::Art { id: Some(id) } => format!("{}?item={}", ART, id)
                    .parse()
                    .expect("valid arg_str in RequestIntent"),
                RequestIntent::Art { id: None } => PathAndQuery::from_static(ART),
                RequestIntent::Playlist => PathAndQuery::from_static(PLAYLIST_JSON),
            };
            Self {
                path_and_query,
                method: Method::GET,
            }
        }
    }
    /// Execution context for [`RequestIntent`]s
    pub(crate) struct Context(Client, Authority);
    impl Context {
        pub fn new(credentials: Credentials) -> Self {
            const NO_USER: &str = "";
            let Credentials {
                password,
                authority,
            } = credentials;
            let client = Client::builder()
                .basic_auth(NO_USER, Some(&password))
                .finish();
            Self(client, authority)
        }
        pub async fn run<'a, 'b>(&self, request: RequestIntent<'a, 'b>) {
            let RequestInfo {
                path_and_query,
                method,
            } = request.into();
            let uri = self.uri_from(path_and_query);
            println!("{}: {}", method, uri);
            let res = self.0.request(method, uri).send().await;
            dbg!(&res);
            //TODO process response internally, only respond to consumer if requested
            res.expect("it always works flawlessly? (fixme)");
        }
        fn uri_from(&self, path_and_query: PathAndQuery) -> Uri {
            Uri::builder()
                .scheme("http")
                .authority(self.1.clone())
                .path_and_query(path_and_query)
                .build()
                .expect("internally-generated URI is valid")
        }
    }
}

use tokio::sync::mpsc::Receiver;
/// Processor for VLC commands and queries
#[derive(Default)]
pub struct Controller;
impl Controller {
    /// Executes the specified commands
    pub async fn run(&self, credentials: Credentials, mut commands: Receiver<Command>) {
        let context = Context::new(credentials);
        while let Some(command) = commands.recv().await {
            dbg!(&command);
            let request = self.encode(command);
            dbg!(&request);
            context.run(request).await;
        }
        println!("context ended!");
    }
    /// Creates a request for the specified command
    #[allow(clippy::unused_self)] // TODO
    pub fn encode(&self, command: Command) -> RequestIntent {
        match command {
            Command::PlaybackResume => RequestIntent::status("pl_forceresume"),
            Command::PlaybackPause => RequestIntent::status("pl_forcepause"),
            Command::PlaybackStop => RequestIntent::status("pl_stop"),
            // _ => todo!(),
        }
    }
    // TODO
    // fn query(&self, query: Query, output: Sender<()>) -> RequestInfo {
    //     todo!()
    // }
}

/// Error obtaining a sepecific environment variable
#[derive(Debug)]
pub struct EnvError(&'static str, std::env::VarError);

/// Host, Port, and Password for connecting to the VLC instance
#[derive(Debug)]
pub struct Credentials {
    /// Password string (plaintext)
    pub password: String,
    /// Host and Port
    pub authority: web::Authority,
}
impl Credentials {
    /// Attempts to construct an authority from environment variables `VLC_HOST`:`VLC_PORT`
    ///
    /// # Errors
    /// Returns an error if the environment args are invalid
    ///
    pub fn try_from_env() -> Result<Result<Credentials, web::InvalidUri>, EnvError> {
        fn get_env(key: &'static str) -> Result<String, EnvError> {
            std::env::var(key).map_err(|e| EnvError(key, e))
        }
        const ENV_VLC_HOST: &str = "VLC_HOST";
        const ENV_VLC_PORT: &str = "VLC_PORT";
        const ENV_VLC_PASSWORD: &str = "VLC_PASSWORD";
        let host = get_env(ENV_VLC_HOST)?;
        let port = get_env(ENV_VLC_PORT)?;
        let password = get_env(ENV_VLC_PASSWORD)?;
        let authority = {
            let host_port: &str = &format!("{host}:{port}", host = host, port = port);
            web::Authority::try_from(host_port)
        };
        Ok(authority.map(|authority| Credentials {
            password,
            authority,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::web::*;
    use super::*;
    fn assert_encode_simple(cmd: Command, expected: RequestInfo) {
        let controller = Controller::default();
        let request = controller.encode(cmd);
        assert_eq!(RequestInfo::from(request), expected);
    }
    #[test]
    fn execs_simple() {
        assert_encode_simple(
            Command::PlaybackResume,
            RequestInfo {
                path_and_query: "/requests/status.json?command=pl_forceresume"
                    .parse()
                    .unwrap(),
                method: Method::GET,
            },
        );
        assert_encode_simple(
            Command::PlaybackPause,
            RequestInfo {
                path_and_query: "/requests/status.json?command=pl_forcepause"
                    .parse()
                    .unwrap(),
                method: Method::GET,
            },
        );
        assert_encode_simple(
            Command::PlaybackStop,
            RequestInfo {
                path_and_query: "/requests/status.json?command=pl_stop".parse().unwrap(),
                method: Method::GET,
            },
        );
    }
}
