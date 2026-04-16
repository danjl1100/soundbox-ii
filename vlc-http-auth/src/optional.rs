// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Helpers for combining optional parts to build [`AuthInput`]

use crate::{AuthInput, Host, Password, Port};

/// Option parts to be combined to form a whole [`AuthInput`]
#[derive(Clone, Debug)]
pub struct AuthInputOptional {
    /// Password string (plaintext)
    pub vlc_password: Option<Password>,
    /// Host string
    pub vlc_host: Option<Host>,
    /// Port number
    pub vlc_port: Option<Port>,
}
impl AuthInputOptional {
    /// Fills any missing portions of `self` from `other`
    ///
    /// Field-wise equivalent of [`Option::or`]
    #[must_use]
    pub fn or(self, other: Self) -> Self {
        let Self {
            vlc_password,
            vlc_host,
            vlc_port,
        } = self;
        Self {
            vlc_password: vlc_password.or(other.vlc_password),
            vlc_host: vlc_host.or(other.vlc_host),
            vlc_port: vlc_port.or(other.vlc_port),
        }
    }
    /// Fills any missing portions of `self` from `other`
    ///
    /// Field-wise equivalent of [`Option::unwrap_or`]
    #[must_use]
    pub fn unwrap_or(self, other: AuthInput) -> AuthInput {
        let Self {
            vlc_password,
            vlc_host,
            vlc_port,
        } = self;
        AuthInput {
            vlc_password: vlc_password.unwrap_or(other.vlc_password),
            vlc_host: vlc_host.unwrap_or(other.vlc_host),
            vlc_port: vlc_port.unwrap_or(other.vlc_port),
        }
    }
}
impl TryFrom<AuthInputOptional> for AuthInput {
    type Error = MissingPartsError;
    fn try_from(value: AuthInputOptional) -> Result<Self, MissingPartsError> {
        use MissingParts as E;

        let AuthInputOptional {
            mut vlc_password,
            mut vlc_host,
            mut vlc_port,
        } = value;

        let parts = match (&mut vlc_host, &mut vlc_password, &mut vlc_port) {
            (None, None, None) => E::All,
            //
            (None::<Host>, None::<Password>, _) => E::HostPassword,
            (None::<Host>, _, None::<Port>) => E::HostPort,
            (_, None::<Password>, None::<Port>) => E::PasswordPort,
            //
            (None::<Host>, _, _) => E::Host,
            (_, None::<Password>, _) => E::Password,
            (_, _, None::<Port>) => E::Port,
            //
            (host @ Some(_), password @ Some(_), port @ Some(_)) => {
                let ((vlc_host, vlc_password), vlc_port) = host
                    .take()
                    .zip(password.take())
                    .zip(port.take())
                    .expect("all matched some");

                return Ok(Self {
                    vlc_password,
                    vlc_host,
                    vlc_port,
                });
            }
        };

        Err(MissingPartsError {
            parts,
            value: AuthInputOptional {
                vlc_password,
                vlc_host,
                vlc_port,
            },
        })
    }
}

/// Error constructing [`AuthInput`] from [`AuthInputOptional`]
#[derive(Clone, Debug)]
pub struct MissingPartsError {
    parts: MissingParts,
    value: AuthInputOptional,
}
#[derive(Clone, Debug)]
enum MissingParts {
    All,
    HostPassword,
    HostPort,
    PasswordPort,
    Host,
    Password,
    Port,
}
impl MissingPartsError {
    /// Returns the original [`AuthInputOptional`] value that the error describes
    #[must_use]
    pub fn into_inner(self) -> AuthInputOptional {
        self.value
    }
}
impl std::error::Error for MissingPartsError {}
impl std::fmt::Display for MissingPartsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { parts, value: _ } = self;
        let parts = match parts {
            MissingParts::All => "host, password, and port",
            MissingParts::HostPassword => "host and password",
            MissingParts::HostPort => "host and port",
            MissingParts::PasswordPort => "password and port",
            MissingParts::Host => "host",
            MissingParts::Password => "password",
            MissingParts::Port => "port",
        };
        write!(f, "incomplete VLC auth, missing {parts}")
    }
}
