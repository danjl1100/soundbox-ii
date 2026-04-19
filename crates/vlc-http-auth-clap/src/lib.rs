// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Input authentication parameters to the VLC instance using [`clap`]

use vlc_http_auth::{AuthInput, Host, Password, Port, optional::AuthInputOptional};

pub use ::clap as clap_crate;

/// Creates [`AuthInput`] from [`clap`] inputs
#[derive(Clone, clap::Args, Debug)]
pub struct ClapAuthInput {
    /// Password string (plaintext)
    #[clap(long, env = "VLC_PASSWORD")]
    pub vlc_password: String,
    /// Host string
    #[clap(long, env = "VLC_HOST")]
    pub vlc_host: String,
    /// Port number
    #[clap(long, env = "VLC_PORT")]
    pub vlc_port: u16,
}
impl From<ClapAuthInput> for AuthInput {
    fn from(value: ClapAuthInput) -> Self {
        let ClapAuthInput {
            vlc_password,
            vlc_host,
            vlc_port,
        } = value;
        Self {
            vlc_password: Password(vlc_password),
            vlc_host: Host(vlc_host),
            vlc_port: Port(vlc_port),
        }
    }
}

/// Option parts from [`clap`] inputs, to be combined
/// to form a whole [`AuthInput`]
#[derive(Clone, clap::Args, Debug)]
pub struct ClapAuthInputOptional {
    /// Password string (plaintext)
    #[clap(long, env = "VLC_PASSWORD")]
    pub vlc_password: Option<String>,
    /// Host string
    #[clap(long, env = "VLC_HOST")]
    pub vlc_host: Option<String>,
    /// Port number
    #[clap(long, env = "VLC_PORT")]
    pub vlc_port: Option<u16>,
}
impl ClapAuthInputOptional {
    /// Helpful to guide inference through a series of conversions, for example:
    /// [`ClapAuthInput`] -> `into_common()` -> [`AuthInputOptional`] -> `try_into()` -> [`AuthInput`]
    #[must_use]
    pub fn into_common(self) -> AuthInputOptional {
        self.into()
    }
}
impl From<ClapAuthInputOptional> for AuthInputOptional {
    fn from(value: ClapAuthInputOptional) -> Self {
        let ClapAuthInputOptional {
            vlc_password,
            vlc_host,
            vlc_port,
        } = value;
        Self {
            vlc_password: vlc_password.map(Password),
            vlc_host: vlc_host.map(Host),
            vlc_port: vlc_port.map(Port),
        }
    }
}
