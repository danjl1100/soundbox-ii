// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
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
pub struct RawCredentials {
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
    /// use vlc_http::auth::{RawCredentials, PartialConfig};
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
    /// assert!(RawCredentials::try_from_partial(result).is_ok());
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
impl RawCredentials {
    /// Parses the specified port string
    ///
    /// # Errors
    /// Returns an error if the parsing fails
    ///
    pub fn parse_port(port_str: String) -> Result<u16, ParsePortError> {
        u16::from_str(&port_str).map_err(|err| (port_str, err))
    }
    /// Attempts to construct `RawCredentials` from the specified `PartialConfig`
    ///
    /// # Example
    ///
    /// ```
    /// # use vlc_http::auth::{RawCredentials, PartialConfig};
    /// let partial_good = PartialConfig::<()> {
    ///     password: Ok("1234".to_string()),
    ///     host: Ok("localhost".to_string()),
    ///     port: Ok("22".to_string()),
    /// };
    /// let config = RawCredentials {
    ///     password: "1234".to_string(),
    ///     host: "localhost".to_string(),
    ///     port: 22,
    /// };
    /// assert_eq!(RawCredentials::try_from_partial(partial_good), Ok(Ok(config)));
    /// ```
    ///
    /// # Errors
    ///
    /// Returns a `PartialConfig` if one or more fields are missing
    /// ```
    /// # use vlc_http::auth::{RawCredentials, PartialConfig};
    /// let partial_err = PartialConfig {
    ///     password: Ok("secret".to_string()),
    ///     host: Err("not sure why not"),
    ///     port: Ok("22".to_string()),
    /// };
    /// assert_eq!(RawCredentials::try_from_partial(partial_err.clone()), Err(partial_err));
    /// ```
    ///
    /// Returns an `Ok(Err(ParsePortError))` if the port string is invalid
    /// ```
    /// # use vlc_http::auth::{RawCredentials, PartialConfig};
    /// let non_numeric_port = "not a number".to_string();
    /// let partial_err_port = PartialConfig::<()> {
    ///     password: Ok("1234".to_string()),
    ///     host: Ok("localhost".to_string()),
    ///     port: Ok(non_numeric_port.clone()),
    /// };
    /// let num_err = RawCredentials::parse_port(non_numeric_port.clone()).unwrap_err();
    /// assert_eq!(RawCredentials::try_from_partial(partial_err_port), Ok(Err(num_err)));
    /// ```
    pub fn try_from_partial<E>(
        partial: PartialConfig<E>,
    ) -> Result<Result<Self, ParsePortError>, PartialConfig<E>> {
        match partial {
            PartialConfig {
                password: Ok(password),
                host: Ok(host),
                port: Ok(port),
            } => Ok(Self::parse_port(port).map(|port| RawCredentials {
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
        writeln!(f, "RawCredentials {{")?;
        write_val(f, "host    ", &self.host)?;
        write_val(f, "port    ", &self.port)?;
        write_val(f, "password", &self.password)?;
        write!(f, "}}")
    }
}
impl TryFrom<RawCredentials> for Credentials {
    type Error = (String, InvalidUri);
    fn try_from(config: RawCredentials) -> Result<Self, Self::Error> {
        let RawCredentials {
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
    /// Returns a string describing the authority (host:port)
    pub fn authority_str(&self) -> &str {
        self.authority.as_str()
    }
}
