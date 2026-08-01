// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use eyre::Context as _;
use validator::Validate as _;

#[derive(Debug, Clone, serde::Deserialize, validator::Validate)]
pub struct Config {
    pub bind_ip: std::net::IpAddr,
    pub port: u16,
    pub script_write_port: Option<std::path::PathBuf>,
}
impl Config {
    /// Loads the configuration from the environment
    ///
    /// # Errors
    /// Returns an error if the deserialization or validation fails
    pub fn from_env() -> eyre::Result<Self> {
        let config = ::config::Config::builder()
            .add_source(
                ::config::Environment::default()
                    .prefix("BEET_PUSHER_WEBUI")
                    .separator("__"),
            )
            .set_default("bind_ip", "127.0.0.1")
            .and_then(|b| b.set_default("port", "0"))
            .context("failed to init default configuration")?
            .build()
            .context("failed to build configuration")?;

        let parsed: Config = config
            .try_deserialize()
            .context("failed to deserialize configuration")?;

        parsed.validate()?;

        Ok(parsed)
    }
}
