// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Input authentication parameters to the VLC instance using [`clap`]

use vlc_http_auth::AuthInput;

pub use ::clap as clap_crate;

/// Creates [`AuthInput`] from [`clap`] inputs
#[derive(Clone, clap::Args, Debug)]
pub struct ClapAuthInput {
    /// Password string (plaintext)
    #[clap(long, env = "VLC_PASSWORD")]
    pub password: String,
    /// Host string
    #[clap(long, env = "VLC_HOST")]
    pub host: String,
    /// Port number
    #[clap(long, env = "VLC_PORT")]
    pub port: u16,
}
impl From<ClapAuthInput> for AuthInput {
    fn from(value: ClapAuthInput) -> Self {
        let ClapAuthInput {
            password,
            host,
            port,
        } = value;
        Self {
            password,
            host,
            port,
        }
    }
}
