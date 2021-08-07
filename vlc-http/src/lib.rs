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

use tokio::sync::{mpsc, oneshot};

/// Sender for an action result
pub type ResultSender<T> = oneshot::Sender<Result<T, Error>>;
/// Receiver for an action result
pub type ResultReceiver<T> = oneshot::Receiver<Result<T, Error>>;

/// Action available to be `run()`, with `Sender<T>` for returning the result
#[must_use]
#[derive(Debug)]
pub enum Action {
    /// [`Command`] with `Sender` for the result
    Command(Command, ResultSender<()>),
    /// [`Query`] with a `Sender` for the result
    QueryPlaybackStatus(ResultSender<PlaybackStatus>),
    /// [`Query`] with a `Sender` for the result
    QueryPlaylistInfo(ResultSender<PlaylistInfo>),
}
impl Action {
    /// Constructs a [`Query::PlaybackStatus`] action variant, with the corresponding
    /// [`ResultReceiver`]
    pub fn query_playback_status() -> (Self, ResultReceiver<PlaybackStatus>) {
        let (result_tx, result_rx) = oneshot::channel();
        let action = Self::QueryPlaybackStatus(result_tx);
        (action, result_rx)
    }
    /// Constructs a [`Query::PlaylistInfo`] action variant, with the corresponding
    /// [`ResultReceiver`]
    pub fn query_playlist_info() -> (Self, ResultReceiver<PlaylistInfo>) {
        let (result_tx, result_rx) = oneshot::channel();
        let action = Self::QueryPlaylistInfo(result_tx);
        (action, result_rx)
    }
}
impl Command {
    /// Constructs an [`Action`] from the Command, with the corresponding [`ResultReceiver`]
    pub fn to_action_rx(self) -> (Action, ResultReceiver<()>) {
        let (result_tx, result_rx) = oneshot::channel();
        let action = Action::Command(self, result_tx);
        (action, result_rx)
    }
}
impl std::fmt::Display for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Command(command, _) => write!(f, "{:?}", command),
            Self::QueryPlaybackStatus(_) => write!(f, "{:?}", Query::PlaybackStatus),
            Self::QueryPlaylistInfo(_) => write!(f, "{:?}", Query::PlaylistInfo),
        }
    }
}

pub use command::{Command, Query};
mod command;

mod request;

pub use vlc_responses::{PlaybackStatus, PlaylistInfo};
pub mod vlc_responses;

pub(crate) use context::Context;
mod context {
    use super::{auth::Credentials, command::RequestIntent, request::RequestInfo};
    use hyper::{
        body::Body, client::Builder as ClientBuilder, Client as HyperClient,
        Request as HyperRequest,
    };
    type Client = HyperClient<hyper::client::connect::HttpConnector, Body>;
    type Request = HyperRequest<Body>;

    /// Execution context for [`RequestIntent`]s
    pub(crate) struct Context(Client, Credentials);
    impl Context {
        pub fn new(credentials: Credentials) -> Self {
            let client = ClientBuilder::default().build_http();
            Self(client, credentials)
        }
        pub async fn run<'a, 'b>(
            &self,
            request_intent: &RequestIntent<'a, 'b>,
        ) -> Result<hyper::Response<Body>, hyper::Error> {
            let request_info = RequestInfo::from(request_intent);
            Ok(self.run_retry_loop(request_info).await?)
        }
        async fn run_retry_loop(
            &self,
            request: RequestInfo,
        ) -> Result<hyper::Response<Body>, hyper::Error> {
            use backoff::{future::retry, ExponentialBackoff};
            use tokio::time::Duration;
            let backoff_config = ExponentialBackoff {
                current_interval: Duration::from_millis(50),
                initial_interval: Duration::from_millis(50),
                multiplier: 4.0,
                max_interval: Duration::from_secs(2),
                max_elapsed_time: Some(Duration::from_secs(10)),
                ..ExponentialBackoff::default()
            };
            retry(backoff_config, || async {
                let request = self.request_from(request.clone()); //TODO: avoid expensive clone?
                Ok(self.0.request(request).await?)
            })
            .await
        }
        fn request_from(&self, info: RequestInfo) -> Request {
            let RequestInfo {
                path_and_query,
                method,
            } = info;
            let uri = self
                .1
                .uri_builder()
                .path_and_query(path_and_query)
                .build()
                .expect("internally-generated URI is valid");
            self.1
                .request_builder()
                .uri(uri)
                .method(method)
                .body(Body::empty())
                .expect("internally-generated URI and Method is valid")
        }
    }
}

pub use auth::{Config, Credentials};
pub mod auth {
    //! Primitives for authorization / method of connecting to VLC server
    use http::{
        request::Builder as RequestBuilder,
        uri::{Authority, Builder as UriBuilder, InvalidUri},
    };

    use std::convert::TryFrom;
    use std::str::FromStr;

    /// Envinronmental variable for VLC host
    pub const ENV_VLC_HOST: &str = "VLC_HOST";
    /// Envinronmental variable for VLC port
    pub const ENV_VLC_PORT: &str = "VLC_PORT";
    /// Envinronmental variable for VLC password
    pub const ENV_VLC_PASSWORD: &str = "VLC_PASSWORD";

    /// Error obtaining a sepecific environment variable
    #[derive(Debug)]
    pub struct EnvError(&'static str, std::env::VarError);
    impl std::fmt::Display for EnvError {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            use std::env::VarError;
            let reason = match self.1 {
                VarError::NotPresent => "missing",
                VarError::NotUnicode(_) => "non-unicode",
            };
            write!(f, "{} environment variable \"{}\"", reason, self.0)
        }
    }

    /// Configuration for connecting to the VLC instance
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Config {
        /// Password string (plaintext)
        pub password: String,
        /// Host string
        pub host: String,
        /// Port number
        pub port: u16,
    }
    /// Partial configuration for connecting to the VLC instance
    #[must_use]
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct PartialConfig<E> {
        /// Password string (plaintext)
        pub password: Result<String, E>,
        /// Host string
        pub host: Result<String, E>,
        /// Port string
        pub port: Result<String, E>,
    }
    impl PartialConfig<EnvError> {
        /// Constructs a `PartialConfig` from environment variables
        ///
        /// # Errors
        /// Returns an error if the `port` value is present, but not a valid number
        ///
        pub fn from_env() -> Self {
            fn get_env(key: &'static str) -> Result<String, EnvError> {
                std::env::var(key).map_err(|e| EnvError(key, e))
            }
            Self {
                host: get_env(ENV_VLC_HOST),
                port: get_env(ENV_VLC_PORT),
                password: get_env(ENV_VLC_PASSWORD),
            }
        }
    }
    impl<E> PartialConfig<E> {
        /// Returns `true` if the `PartialConfig` is empty
        ///
        /// ```
        /// use vlc_http::auth::PartialConfig;
        ///
        /// let empty = PartialConfig {
        ///     host: Err(()),
        ///     port: Err(()),
        ///     password: Err(()),
        /// };
        /// assert_eq!(empty.is_empty(), true);
        ///
        /// let partial_host = PartialConfig {
        ///     host: Ok("host".to_string()),
        ///     ..empty.clone()
        /// };
        /// let partial_port = PartialConfig {
        ///     port: Ok("port".to_string()),
        ///     ..empty.clone()
        /// };
        /// let partial_pass = PartialConfig {
        ///     password: Ok("password".to_string()),
        ///     ..empty
        /// };
        /// assert_eq!(partial_host.is_empty(), false);
        /// assert_eq!(partial_port.is_empty(), false);
        /// assert_eq!(partial_pass.is_empty(), false);
        /// ```
        pub fn is_empty(&self) -> bool {
            self.password.is_err() && self.host.is_err() && self.port.is_err()
        }
        /// Moves all `Ok` fields from `other` to `self`
        ///
        /// ```
        /// use vlc_http::auth::{Config, PartialConfig};
        /// use std::convert::TryFrom;
        ///
        /// let priority = PartialConfig {
        ///     host: Ok("this value overrides value".to_string()), // *
        ///     port: Ok("this value overrides Err".to_string()), // *
        ///     password: Err("unused Err"),
        /// };
        /// let base = PartialConfig {
        ///     host: Ok("value overrides this value".to_string()),
        ///     port: Err("value overrides this Err".to_string()),
        ///     password: Ok("Err does NOT override this value".to_string()), // *
        /// };
        ///
        /// let result = base.override_with(priority);
        /// assert_eq!(result, PartialConfig {
        ///     host: Ok("this value overrides value".to_string()),
        ///     port: Ok("this value overrides Err".to_string()),
        ///     password: Ok("Err does NOT override this value".to_string()),
        /// });
        /// assert!(Config::try_from_partial(result).is_ok());
        /// ```
        pub fn override_with<U>(mut self, other: PartialConfig<U>) -> Self {
            if let Ok(host) = other.host {
                self.host = Ok(host);
            }
            if let Ok(port) = other.port {
                self.port = Ok(port);
            }
            if let Ok(password) = other.password {
                self.password = Ok(password);
            }
            self
        }
    }
    type ParsePortError = (String, std::num::ParseIntError);
    impl Config {
        /// Parses the specified port string
        ///
        /// # Errors
        /// Returns an error if the parsing fails
        ///
        pub fn parse_port(port_str: String) -> Result<u16, ParsePortError> {
            u16::from_str(&port_str).map_err(|err| (port_str, err))
        }
        /// Attempts to construct `Config` from the specified `PartialConfig`
        ///
        /// # Errors
        /// Returns a `PartialConfig` if one or more fields are missing
        /// Returns an `Ok(Err(ParsePortErrpr))` if the port string is invalid
        pub fn try_from_partial<E>(
            partial: PartialConfig<E>,
        ) -> Result<Result<Self, ParsePortError>, PartialConfig<E>> {
            match partial {
                PartialConfig {
                    password: Ok(password),
                    host: Ok(host),
                    port: Ok(port),
                } => Ok(Self::parse_port(port).map(|port| Config {
                    password,
                    host,
                    port,
                })),
                partial => Err(partial),
            }
        }
    }
    impl<E> std::fmt::Display for PartialConfig<E>
    where
        E: std::fmt::Display,
    {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            fn write_val<T, E>(
                f: &mut std::fmt::Formatter,
                label: &str,
                val: &Result<T, E>,
            ) -> std::fmt::Result
            where
                T: std::fmt::Display,
                E: std::fmt::Display,
            {
                match val {
                    Ok(val) => writeln!(f, "\t{}\t\"{}\"", label, val),
                    Err(err) => writeln!(f, "\t{}\tError: {}", label, err),
                }
            }
            writeln!(f, "Config {{")?;
            write_val(f, "host    ", &self.host)?;
            write_val(f, "port    ", &self.port)?;
            write_val(f, "password", &self.password)?;
            write!(f, "}}")
        }
    }
    impl TryFrom<Config> for Credentials {
        type Error = (String, InvalidUri);
        fn try_from(config: Config) -> Result<Self, Self::Error> {
            let Config {
                password,
                host,
                port,
            } = config;
            let user_pass = format!(":{}", password);
            let auth = format!("Basic {}", base64::encode(user_pass));
            let host_port: String = format!("{host}:{port}", host = host, port = port);
            Authority::from_str(&host_port)
                .map_err(|uri_err| (host_port, uri_err))
                .map(|authority| Credentials { auth, authority })
        }
    }
    /// Credential information for connecting to the VLC instance
    #[derive(Debug)]
    pub struct Credentials {
        /// Bearer string (base64 encoded password with prefix)
        auth: String,
        /// Host and Port
        authority: Authority,
    }
    impl Credentials {
        /// Constructs a [`UriBuilder`] from the credential info
        pub fn uri_builder(&self) -> UriBuilder {
            UriBuilder::new()
                .scheme("http")
                .authority(self.authority.clone())
        }
        /// Constructs a [`RequestBuilder`] from the credential info
        pub fn request_builder(&self) -> RequestBuilder {
            RequestBuilder::new().header("Authorization", &self.auth)
        }
    }
}

/// Error from the `run()` function
#[derive(Debug)]
pub enum Error {
    /// Hyper client-side error
    Hyper(hyper::Error),
    /// Deserialization error
    Serde(serde_json::Error),
}
impl From<hyper::Error> for Error {
    fn from(err: hyper::Error) -> Self {
        Self::Hyper(err)
    }
}
impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Self::Serde(err)
    }
}

/// Executes the specified commands
pub async fn run(credentials: Credentials, mut commands: mpsc::Receiver<Action>) {
    let context = Context::new(credentials);
    while let Some(action) = commands.recv().await {
        match action {
            Action::Command(command, result_tx) => {
                let request = command.into();
                let result = context
                    .run(&request)
                    .await
                    .map(|_| ())
                    .map_err(|e| e.into());
                send_result(result, result_tx);
            }
            Action::QueryPlaybackStatus(result_tx) => {
                let request = Query::PlaybackStatus.into();
                let response = context.run(&request).await;
                let result = parse_body_json(response, PlaybackStatus::from_slice).await;
                send_result(result, result_tx);
            }
            Action::QueryPlaylistInfo(result_tx) => {
                let request = Query::PlaylistInfo.into();
                let response = context.run(&request).await;
                let result = parse_body_json(response, PlaylistInfo::from_slice).await;
                send_result(result, result_tx);
            }
        };
    }
    println!("vlc_http::run() - context ended!");
}
async fn parse_body_json<F, T, E>(
    result: Result<hyper::Response<hyper::Body>, hyper::Error>,
    map_fn: F,
) -> Result<T, Error>
where
    F: FnOnce(&[u8]) -> Result<T, E>,
    Error: From<E>,
{
    match result {
        Ok(response) => hyper::body::to_bytes(response.into_body())
            .await
            .map_err(|err| err.into())
            .and_then(|bytes| Ok(map_fn(&bytes)?)),
        Err(err) => Err(err.into()),
    }
}
fn send_result<T>(result: Result<T, Error>, result_tx: ResultSender<T>)
where
    T: std::fmt::Debug,
{
    let send_result = result_tx.send(result);
    if let Err(send_err) = send_result {
        println!("WARNING: result_tx send error: {:?}", send_err);
    }
}
