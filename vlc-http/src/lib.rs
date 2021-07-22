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

/// Control commands for VLC
#[derive(Debug, Clone)]
#[allow(clippy::pub_enum_variant_names)]
pub enum Command {
    /// Add the specified item to the playlist
    PlaylistAdd {
        /// Path to the file to enqueue
        uri: String,
    },
    /// Play the specified item in the playlist
    PlaylistPlay {
        /// Identifier of the playlist item
        item_id: Option<String>,
    },
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
    // /// Set the playback speed (unit scale, 1.0 = normal speed)
    // PlaybackSpeed(f64),
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

pub use web::Credentials;
pub(crate) use web::{Context, RequestIntent};
pub mod web {
    //! HTTP-specific primitives (interchange for test purposes)
    pub use awc::{
        http::{
            uri::{Authority, InvalidUri, PathAndQuery, Uri},
            Method,
        },
        Client,
    };

    /// VLC backend request information
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
            args: Vec<(&'b str, String)>,
        },
        Art {
            id: Option<String>,
        },
        Playlist {
            command: &'a str,
            args: Vec<(&'b str, String)>,
        },
    }
    impl<'a, 'b> RequestIntent<'a, 'b> {
        pub(crate) fn status(command: &'a str) -> Self {
            Self::Status {
                command,
                args: vec![],
            }
        }
    }
    impl<'a, 'b> From<RequestIntent<'a, 'b>> for RequestInfo {
        fn from(intent: RequestIntent<'a, 'b>) -> Self {
            const STATUS_JSON: &str = "/requests/status.json";
            const PLAYLIST_JSON: &str = "/requests/playlist.json";
            const ART: &str = "/art";
            let path_and_query = match intent {
                RequestIntent::Status { command, args } => {
                    Self::format_cmd_args(STATUS_JSON, command, args)
                }
                RequestIntent::Playlist { command, args } => {
                    Self::format_cmd_args(PLAYLIST_JSON, command, args)
                }
                RequestIntent::Art { id: Some(id) } => Self::format_path_query(
                    ART,
                    &Self::query_builder().append_pair("item", &id).finish(),
                ),
                RequestIntent::Art { id: None } => PathAndQuery::from_static(ART),
            };
            Self {
                path_and_query,
                method: Method::GET,
            }
        }
    }
    impl RequestInfo {
        fn query_builder() -> form_urlencoded::Serializer<'static, String> {
            form_urlencoded::Serializer::new(String::new())
        }
        fn format_cmd_args(path: &str, command: &str, args: Vec<(&str, String)>) -> PathAndQuery {
            let query = Self::query_builder()
                .append_pair("command", command)
                .extend_pairs(args)
                .finish();
            Self::format_path_query(path, &query)
        }
        fn format_path_query(path: &str, query: &str) -> PathAndQuery {
            format!("{path}?{query}", path = path, query = query)
                .parse()
                .expect("valid urlencoded args")
        }
    }
    /// Execution context for [`RequestIntent`]s
    pub(crate) struct Context(Client, Credentials);
    impl Context {
        pub fn new(credentials: Credentials) -> Self {
            let client = credentials.client_builder().finish();
            Self(client, credentials)
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
            self.1
                .uri_builder()
                .path_and_query(path_and_query)
                .build()
                .expect("internally-generated URI is valid")
        }
    }

    pub use auth::Credentials;
    pub mod auth {
        //! Primitives for authorization / method of connecting to VLC server
        use awc::http::uri::Builder as UriBuilder;
        use awc::{
            http::uri::{Authority, InvalidUri},
            ClientBuilder,
        };
        use std::convert::TryFrom;

        /// Error obtaining a sepecific environment variable
        #[derive(Debug)]
        pub struct EnvError(&'static str, std::env::VarError);

        /// Credential information for connecting to the VLC instance
        #[derive(Debug)]
        pub struct Credentials {
            /// Password string (plaintext)
            pub password: String,
            /// Host and Port
            pub authority: Authority,
        }
        impl Credentials {
            /// Attempts to construct an authority from environment variables
            ///
            /// # Errors
            /// Returns an error if the environment variables are missing or invalid
            ///
            pub fn try_from_env() -> Result<Result<Credentials, InvalidUri>, EnvError> {
                fn get_env(key: &'static str) -> Result<String, EnvError> {
                    std::env::var(key).map_err(|e| EnvError(key, e))
                }
                const ENV_VLC_HOST: &str = "VLC_HOST";
                const ENV_VLC_PORT: &str = "VLC_PORT";
                const ENV_VLC_PASSWORD: &str = "VLC_PASSWORD";
                let host = get_env(ENV_VLC_HOST)?;
                let port = get_env(ENV_VLC_PORT)?;
                let password = get_env(ENV_VLC_PASSWORD)?;
                let host_port: &str = &format!("{host}:{port}", host = host, port = port);
                Ok(Authority::try_from(host_port).map(|authority| Credentials {
                    password,
                    authority,
                }))
            }
            /// Constructs a [`ClientBuilder`] from the credential info
            pub fn client_builder(&self) -> ClientBuilder {
                self.into()
            }
            /// Constructs a [`UriBuilder`] from the credential info
            pub fn uri_builder(&self) -> UriBuilder {
                self.into()
            }
        }
        impl<'a> From<&'a Credentials> for ClientBuilder {
            fn from(credentials: &'a Credentials) -> ClientBuilder {
                const NO_USER: &str = "";
                let Credentials { password, .. } = credentials;
                ClientBuilder::new().basic_auth(NO_USER, Some(&password))
            }
        }
        impl<'a> From<&'a Credentials> for UriBuilder {
            fn from(credentials: &'a Credentials) -> UriBuilder {
                let Credentials { authority, .. } = credentials;
                UriBuilder::new()
                    .scheme("http")
                    .authority(authority.clone())
            }
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
            Command::PlaylistAdd { uri } => RequestIntent::Playlist {
                command: "in_enqueue",
                args: vec![("input", uri)],
            },
            Command::PlaylistPlay { item_id } => RequestIntent::Status {
                command: "pl_play",
                args: item_id.map(|id| vec![("id", id)]).unwrap_or_default(),
            },
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
            Command::PlaylistPlay { item_id: None },
            RequestInfo {
                path_and_query: "/requests/status.json?command=pl_play".parse().unwrap(),
                method: Method::GET,
            },
        );
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
    #[test]
    fn exec_url_encoded() {
        assert_encode_simple(
            Command::PlaylistAdd {
                uri: String::from("SENTINEL_ _URI_%^$"),
            },
            RequestInfo {
                path_and_query:
                    "/requests/playlist.json?command=in_enqueue&input=SENTINEL_+_URI_%25%5E%24"
                        .parse()
                        .unwrap(),
                method: Method::GET,
            },
        );
        assert_encode_simple(
            Command::PlaylistPlay {
                item_id: Some(String::from("some id")),
            },
            RequestInfo {
                path_and_query: "/requests/status.json?command=pl_play&id=some+id"
                    .parse()
                    .unwrap(),
                method: Method::GET,
            },
        );
    }
}
