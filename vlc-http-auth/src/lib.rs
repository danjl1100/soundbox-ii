// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Separates the authentication input layer from the core `vlc-http` logic
//!
//! Provides a reexport of [`http`] (intrinsic to authentication)

use base64::{Engine as _, prelude::BASE64_STANDARD};
use std::str::FromStr as _;

/// Re-export the HTTP crate used in [`Auth::authority`]
pub use ::http;

pub use self::newtype::{Host, Password, Port};

pub mod optional;

mod newtype {
    // avoid mix-ups, easier to audit line-by-line

    /// Newtype for the `vlc_password` in [`crate::AuthInput`]
    #[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    #[serde(transparent)]
    pub struct Password(pub String);

    /// Newtype for the `vlc_host` in [`crate::AuthInput`]
    #[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    #[serde(transparent)]
    pub struct Host(pub String);

    /// Newtype for the `vlc_port` in [`crate::AuthInput`]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    #[serde(transparent)]
    pub struct Port(pub u16);

    impl std::fmt::Display for Password {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let Self(inner) = self;
            write!(f, "{inner}")
        }
    }
    impl std::fmt::Display for Host {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let Self(inner) = self;
            write!(f, "{inner}")
        }
    }
    impl std::fmt::Display for Port {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let Self(inner) = self;
            write!(f, "{inner}")
        }
    }
}

/// Input authentication parameters to the VLC instance
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct AuthInput {
    /// Password string (plaintext)
    pub vlc_password: Password,
    /// Host string
    pub vlc_host: Host,
    /// Port number
    pub vlc_port: Port,
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
            vlc_password: password,
            vlc_host: host,
            vlc_port: port,
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
        let Self {
            bearer_credential_plaintext: _, // redact (plaintext, base64-encoded)
            authority,
        } = self;
        // describe the "host:port" only
        write!(f, "{}", authority.as_str())
    }
}
impl std::fmt::Debug for AuthInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            vlc_password: _, // redact (plaintext)
            vlc_host,
            vlc_port,
        } = self;
        f.debug_struct("AuthInput")
            .field("vlc_password", &"[redacted]")
            .field("vlc_host", &vlc_host)
            .field("vlc_port", &vlc_port)
            .finish()
    }
}

impl AuthInput {
    /// Returns a sample value for use in creating template files
    #[must_use]
    pub fn sample_for_templates() -> Self {
        AuthInput {
            vlc_password: Password("password".into()),
            vlc_host: Host("host".into()),
            vlc_port: Port(80),
        }
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
