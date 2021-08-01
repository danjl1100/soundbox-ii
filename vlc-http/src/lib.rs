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

/// Action available to be `run()`, with `Sender<T>` for returning the result
#[must_use]
#[derive(Debug)]
pub enum Action {
    /// `Command` with `Sender` for the result
    Command(Command, oneshot::Sender<Result<(), Error>>),
}
impl Command {
    /// Constructs an [`Action`] from the Command, with the corresponding result [`oneshot::Receiver`]
    pub fn to_action_rx(self) -> (Action, oneshot::Receiver<Result<(), Error>>) {
        let (result_tx, result_rx) = oneshot::channel();
        let action = Action::Command(self, result_tx);
        (action, result_rx)
    }
}

pub use command::Command;
mod command;

mod request;

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
            request_intent: RequestIntent<'a, 'b>,
        ) -> Result<(), hyper::Error> {
            let request_info = RequestInfo::from(request_intent);
            // dbg!(&request_info);
            let _res = self.run_retry_loop(request_info).await?;
            //TODO process response internally, only respond to consumer if requested
            Ok(())
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
        fn request_from(&self, request: RequestInfo) -> Request {
            let RequestInfo {
                path_and_query,
                method,
            } = request;
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
mod auth {
    //! Primitives for authorization / method of connecting to VLC server
    use http::{
        request::Builder as RequestBuilder,
        uri::{Authority, Builder as UriBuilder, InvalidUri},
    };
    // use hyper::client::Builder;

    use std::convert::TryFrom;

    /// Error obtaining a sepecific environment variable
    #[derive(Debug)]
    pub struct EnvError(&'static str, std::env::VarError);

    /// Configuration for connecting to the VLC instance
    pub struct Config {
        /// Password string (plaintext)
        pub password: String,
        /// Host string
        pub host: String,
        /// Port number
        pub port: String,
    }
    impl Config {
        /// Attempts to construct an authority from environment variables
        ///
        /// # Errors
        /// Returns an error if the environment variables are missing or invalid
        ///
        pub fn try_from_env() -> Result<Config, EnvError> {
            fn get_env(key: &'static str) -> Result<String, EnvError> {
                std::env::var(key).map_err(|e| EnvError(key, e))
            }
            const ENV_VLC_HOST: &str = "VLC_HOST";
            const ENV_VLC_PORT: &str = "VLC_PORT";
            const ENV_VLC_PASSWORD: &str = "VLC_PASSWORD";
            let host = get_env(ENV_VLC_HOST)?;
            let port = get_env(ENV_VLC_PORT)?;
            let password = get_env(ENV_VLC_PASSWORD)?;
            Ok(Self {
                password,
                host,
                port,
            })
        }
    }
    impl TryFrom<Config> for Credentials {
        type Error = InvalidUri;
        fn try_from(config: Config) -> Result<Self, Self::Error> {
            let Config {
                password,
                host,
                port,
            } = config;
            let user_pass = format!(":{}", password);
            let auth = format!("Basic {}", base64::encode(user_pass));
            let host_port: &str = &format!("{host}:{port}", host = host, port = port);
            Authority::try_from(host_port).map(|authority| Credentials { auth, authority })
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
}
impl From<hyper::Error> for Error {
    fn from(err: hyper::Error) -> Self {
        Self::Hyper(err)
    }
}

/// Executes the specified commands
pub async fn run(credentials: Credentials, mut commands: mpsc::Receiver<Action>) {
    let context = Context::new(credentials);
    while let Some(action) = commands.recv().await {
        match action {
            Action::Command(command, result_tx) => {
                let request = command.into();
                let result = context.run(request).await;
                if let Err(result_tx_err) = result_tx.send(result.map_err(|e| e.into())) {
                    println!("WARNING: result_tx send error: {:?}", result_tx_err);
                }
            }
        }
    }
    println!("context ended!");
}
