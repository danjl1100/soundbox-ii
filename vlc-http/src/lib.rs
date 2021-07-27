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

pub use command::Command;
mod command;

mod request;

pub(crate) use context::Context;
mod context {
    use super::{auth::Credentials, command::RequestIntent, request::RequestInfo};
    use awc::{
        http::{PathAndQuery, Uri},
        Client,
    };
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
}

pub use auth::Credentials;
mod auth {
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

// /// Processor for VLC commands and queries
// #[derive(Default)]
// pub struct Controller;
// impl Controller {
//     // TODO
//     // fn query(&self, query: Query, output: Sender<()>) -> RequestInfo {
//     //     todo!()
//     // }
// }

use tokio::sync::mpsc::Receiver;
/// Executes the specified commands
pub async fn run(credentials: Credentials, mut commands: Receiver<Command>) {
    let context = Context::new(credentials);
    while let Some(command) = commands.recv().await {
        dbg!(&command);
        let request = command.into();
        dbg!(&request);
        context.run(request).await;
    }
    println!("context ended!");
}
