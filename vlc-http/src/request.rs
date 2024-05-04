// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! HTTP-level primitives (interchange for test purposes)

use base64::{prelude::BASE64_STANDARD, Engine as _};
pub use endpoint::Endpoint;
use http::uri::Authority;
use std::str::FromStr as _;
mod endpoint;

/// Input authentication parameters to the VLC instance
#[derive(Clone)]
pub struct AuthInput {
    /// Password string (plaintext)
    pub password: String,
    /// Host string
    pub host: String,
    /// Port number
    pub port: u16,
}
/// Authentication information to reach a VLC instance
#[derive(Clone)]
pub struct Auth {
    /// Bearer string (base64 encoded password with prefix)
    bearer_credential_plaintext: String,
    /// Host and Port
    authority: Authority,
}
impl Auth {
    /// Converts the authentication input into an optimal format for building HTTP requests
    ///
    /// # Errors
    /// Returns an error if the host URI is invalid
    pub fn new(input: AuthInput) -> Result<Self, InvalidHostUri> {
        let AuthInput {
            password,
            host,
            port,
        } = input;

        // username is blank
        let user_pass = format!(":{password}");
        let bearer_credential_plaintext = format!("Basic {}", BASE64_STANDARD.encode(user_pass));

        let host_port: String = format!("{host}:{port}");
        let authority =
            Authority::from_str(&host_port).map_err(|error| InvalidHostUri { host_port, error })?;

        Ok(Self {
            bearer_credential_plaintext,
            authority,
        })
    }
}
/// Error from an invalid `host` in the [`AuthInput`]
#[derive(Debug)]
pub struct InvalidHostUri {
    host_port: String,
    error: http::uri::InvalidUri,
}
impl std::fmt::Display for InvalidHostUri {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { host_port, error } = self;
        write!(f, "error for host:port {host_port:?}: {error}")
    }
}
impl std::error::Error for InvalidHostUri {}

impl std::fmt::Display for Auth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // describe the "host:port" only (skip base64-encoded password)
        write!(f, "{}", self.authority.as_str())
    }
}
impl Endpoint {
    /// Returns a description of the HTTP request to reach this endpoint with the specified authentication
    pub fn with_auth<'a>(&'a self, auth: &'a Auth) -> RequestInfo<'_> {
        RequestInfo {
            endpoint: self,
            bearer_credential_plaintext: &auth.bearer_credential_plaintext,
            authority: &auth.authority,
        }
    }
}

/// Borrowed information to construct an HTTP request
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequestInfo<'a> {
    /// destination path and query for the request
    endpoint: &'a Endpoint,
    /// Bearer string (base64 encoded password with prefix)
    bearer_credential_plaintext: &'a str,
    /// Host and Port
    authority: &'a Authority,
}
impl RequestInfo<'_> {
    /// Creates an HTTP request
    ///
    /// # Panics
    /// Panics if the internal URI and request generation logic fails [`http`] valdiation checks
    #[must_use]
    pub fn build_http_request(self) -> http::Request<()> {
        const HEADER_AUTHORIZATION: &str = "Authorization";

        let Self {
            endpoint,
            bearer_credential_plaintext,
            authority,
        } = self;

        let uri = http::Uri::builder()
            .scheme("http")
            .authority(authority.clone())
            .path_and_query(endpoint.get_path_and_query())
            .build()
            .expect("internally-generated URI is valid");

        http::Request::builder()
            .header(HEADER_AUTHORIZATION, bearer_credential_plaintext)
            .uri(uri)
            .method(endpoint.get_method())
            .body(())
            .expect("internally-generated URI and Method is valid")
    }
}
