// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Separates the authentication input layer from the core `vlc-http` logic

use base64::{Engine as _, prelude::BASE64_STANDARD};
use std::str::FromStr as _;

/// Re-export the HTTP crate used in [`Auth::authority`]
pub use http;

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
    pub bearer_credential_plaintext: String,
    /// Host and Port
    pub authority: http::uri::Authority,
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
        let authority = http::uri::Authority::from_str(&host_port)
            .map_err(|error| InvalidHostUri { host_port, error })?;

        Ok(Self {
            bearer_credential_plaintext,
            authority,
        })
    }
}
impl std::fmt::Display for Auth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // describe the "host:port" only (skip base64-encoded password)
        write!(f, "{}", self.authority.as_str())
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
