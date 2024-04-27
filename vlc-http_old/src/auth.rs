// Copyright (C) 2021-2023  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Primitives for authorization / method of connecting to VLC server
pub use http::uri::InvalidUri;
use http::{
    request::Builder as RequestBuilder,
    uri::{Authority, Builder as UriBuilder},
};

use std::str::FromStr;

/// Error obtaining a sepecific environment variable
#[derive(Debug)]
pub struct EnvError(&'static str, std::env::VarError);
impl std::fmt::Display for EnvError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        use std::env::VarError;
        let Self(variable, reason) = self;
        let reason = match reason {
            VarError::NotPresent => "missing",
            VarError::NotUnicode(_) => "non-unicode",
        };
        write!(f, "{reason} environment variable \"{variable}\"")
    }
}

/// User-supplied credentials for connecting to the VLC instance
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Credentials {
    /// Password string (plaintext)
    pub password: String,
    /// Host string
    pub host: String,
    /// Port number
    pub port: u16,
}

type ParsePortError = (String, std::num::ParseIntError);
impl Credentials {
    /// Parses the specified port string
    ///
    /// # Errors
    /// Returns an error if the parsing fails
    ///
    pub fn parse_port(port_str: String) -> Result<u16, ParsePortError> {
        u16::from_str(&port_str).map_err(|err| (port_str, err))
    }
}
impl TryFrom<Credentials> for Authorization {
    type Error = (String, InvalidUri);
    fn try_from(config: Credentials) -> Result<Self, Self::Error> {
        let Credentials {
            password,
            host,
            port,
        } = config;
        let user_pass = format!(":{password}");
        let auth = format!("Basic {}", base64::encode(user_pass));
        let host_port: String = format!("{host}:{port}");
        Authority::from_str(&host_port)
            .map_err(|uri_err| (host_port, uri_err))
            .map(|authority| Authorization { auth, authority })
    }
}
/// Low-level authorization information for connecting to the VLC instance
#[derive(Debug, Clone)]
pub struct Authorization {
    /// Bearer string (base64 encoded password with prefix)
    auth: String,
    /// Host and Port
    authority: Authority,
}
impl Authorization {
    /// Constructs a [`UriBuilder`] from the authorization info
    pub fn uri_builder(&self) -> UriBuilder {
        UriBuilder::new()
            .scheme("http")
            .authority(self.authority.clone())
    }
    /// Constructs a [`RequestBuilder`] from the authorization info
    pub fn request_builder(&self) -> RequestBuilder {
        RequestBuilder::new().header("Authorization", &self.auth)
    }
    /// Returns a string describing the authority (host:port)
    pub fn authority_str(&self) -> &str {
        self.authority.as_str()
    }
}
