// Copyright (C) 2021-2023  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Parses command-line arguments

use self::args::{RawArgs, RawArgsUnpacked};
use arg_util::{Input, Source, Value};
use clap::Parser;
use http::uri::InvalidUri;
use shared::{wrapper_enum, IgnoreNever};
use std::net::{AddrParseError, SocketAddr};
use std::path::PathBuf;

// TODO remove this planning info, should be clearly denoted in the RawArg structs
// format is CLI ARGUMENTS [ENV VAR]
//
// - config-file [CONFIG_FILE]
// - VlcHttp:
//   - VLC_HOST [VLC_HOST]
//   - VLC_PASSWORD [VLC_PASSWORD]
//   - VLC_PORT [VLC_PORT]
// - WebServer:
//   - serve
//   - BIND_ADDRESS [BIND_ADDRESS]
//   - STAIC_ASSETS [STATIC_ASSETS]
//   - watch-assets [WATCH_ASSETS=1]
// - Cli:
//   - interactive
//   - run-script
//   - state-file [STATE_FILE]
// - Sequencer:
//   - BEET_CMD [BEET_CMD]
//   - ROOT_FOLDER [ROOT_FOLDER]
//

mod args;

mod env_vars {
    pub const CONFIG_FILE: &str = "CONFIG_FILE";
    // VlcHttp
    pub const VLC_HOST: &str = "VLC_HOST";
    pub const VLC_PORT: &str = "VLC_PORT";
    pub const VLC_PASSWORD: &str = "VLC_PASSWORD";
    // Web Server
    pub const BIND_ADDRESS: &str = "BIND_ADDRESS";
    pub const STATIC_ASSETS: &str = "STATIC_ASSETS";
    pub const WATCH_ASSETS: &str = "WATCH_ASSETS";
    // Cli
    pub const STATE_FILE: &str = "STATE_FILE";
    // Sequencer
    pub const BEET_CMD: &str = "BEET_CMD";
    pub const ROOT_FOLDER: &str = "ROOT_FOLDER";
}

/// Final structured configuration
#[allow(clippy::struct_field_names)] // avoids shadowning module names
pub struct Config {
    /// Configuration for the VLC HTTP interface
    pub vlc_http_config: VlcHttp,
    /// Configuration for the webserver
    /// If `None`, then interactive mode implicitly enabled
    pub web_config: Option<WebServer>,
    /// Configuration for the command line interface
    pub cli_config: Cli,
    /// Configuration for the Sequencer item source(s)
    pub sequencer_config: Sequencer,
}
pub struct VlcHttp(pub vlc_http::Authorization);
pub struct WebServer {
    pub bind_address: SocketAddr,
    pub static_assets: PathBuf,
    pub watch_assets: bool,
}
pub struct Cli {
    /// Forces interactive mode to activate (even if web server enabled)
    force_interactive: bool,
    /// Script file to run
    pub run_script: Option<PathBuf>,
    /// File to load state, then periodically store
    pub state_file: Option<PathBuf>,
}
pub struct Sequencer {
    pub root_folder: sequencer::sources::RootFolder,
    pub beet_cmd: sequencer::sources::Beet,
}
impl Config {
    pub fn is_interactive(&self) -> bool {
        self.web_config.is_none() || self.cli_config.force_interactive
    }
}

wrapper_enum! {
    pub enum Error {
        Usage(UsageError),
        ConfigFile(ConfigFileError),
        VlcHttp(VlcHttpError),
        Web(WebError),
        Sequencer(SequencerError),
    }
    #[derive(Debug)]
    pub enum ConfigFileErrorNoContext {
        Toml(toml::de::Error),
        Io(std::io::Error),
    }
}
pub enum UsageError {
    Clap(String),
    Env { key: &'static str, message: String },
}
#[allow(clippy::module_name_repetitions)]
pub struct ConfigFileError {
    filename: PathBuf,
    error: ConfigFileErrorNoContext,
}
pub enum VlcHttpError {
    Incomplete {
        host: Option<String>,
        port: Option<u16>,
        password: Option<String>,
    },
    InvalidUri {
        url: String,
        error: InvalidUri,
        host_source: Source,
        port_source: Source,
    },
}
pub enum WebError {
    BindAddress {
        address: Value<String>,
        error: AddrParseError,
    },
    StaticAssets {
        folder: Value<String>,
        error: std::io::Error,
    },
}
pub enum SequencerError {
    RootFolder {
        folder: Value<String>,
        error: std::io::Error,
    },
    BeetCommand {
        source: Source,
        error: sequencer::sources::PathError,
    },
}
impl std::fmt::Display for ConfigFileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { filename, error } = self;
        write!(f, "in file {filename:?}")?;
        match error {
            ConfigFileErrorNoContext::Toml(err) => write!(f, "{err}"),
            ConfigFileErrorNoContext::Io(err) => write!(f, "{err}"),
        }
    }
}
impl std::fmt::Display for VlcHttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        const NOTE_CMD_HELP: &str =
            "NOTE: View command-line help (-h) for alternate methods of specifying VLC-HTTP parameters.";
        match self {
            Self::Incomplete {
                host,
                port,
                password,
            } => {
                let host = host.as_ref();
                let password = password.as_ref();
                let err_none_str = "Error: [none]";
                let err_none = || err_none_str.to_string();
                //
                let host_str = host.map_or_else(err_none, |host| format!("{host:?}"));
                let port_str = port.map_or_else(err_none, |port| format!("{port}"));
                let password_str = password.map_or(err_none_str, |_| "[redacted]");
                write!(
                    f,
                    "incomplete VLC-HTTP config {{\n\
                                       \thost:     {host_str}\n\
                                       \tport:     {port_str}\n\
                                       \tpassword: {password_str}\n\
                                       }}\n\
                                       {NOTE_CMD_HELP}"
                )
            }
            Self::InvalidUri {
                url,
                error,
                host_source,
                port_source,
            } => {
                let sources = if host_source == port_source {
                    format!("{host_source}")
                } else {
                    format!("{host_source} and {port_source}")
                };
                write!(
                    f,
                    "invalid VLC-HTTP host/port (from {sources}): {error} {url:?}"
                )
            }
        }
    }
}
impl std::fmt::Display for WebError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WebError::BindAddress { address, error } => {
                write!(f, "invalid bind address {address}: {error}")
            }
            WebError::StaticAssets { folder, error } => {
                write!(f, "invalid static-assets path {folder}: {error}")
            }
        }
    }
}
impl std::fmt::Display for SequencerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SequencerError::RootFolder { folder, error } => {
                write!(f, "invalid root-folder path {folder}: {error}")
            }
            SequencerError::BeetCommand { source, error } => {
                write!(f, "invalid beet command (from {source}): {error}")
            }
        }
    }
}

pub fn render_usage() -> String {
    use clap::CommandFactory;
    RawArgs::command().render_usage()
}
pub fn parse_input() -> Result<Config, Error> {
    let mut cli_args = RawArgs::parse();
    let file_args = if let Some(filename) = cli_args.config_file.take() {
        read_config_file(&filename).map_err(|error| ConfigFileError { filename, error })?
    } else {
        RawArgs::default()
    };

    Input {
        cli_args,
        file_args,
    }
    .try_into()
}
fn read_config_file(config_file: &PathBuf) -> Result<RawArgs, ConfigFileErrorNoContext> {
    let file_contents = std::fs::read_to_string(config_file)?;
    let config = toml::from_str(&file_contents)?;
    Ok(config)
}

impl TryFrom<Input<RawArgs>> for Config {
    type Error = Error;

    fn try_from(input: Input<RawArgs>) -> Result<Self, Self::Error> {
        let RawArgsUnpacked {
            vlc_http_config,
            web_config,
            cli_config,
            sequencer_config,
            serve,
            config_file,
        } = input.into();
        assert_eq!(
            config_file, // used earlier to create Input
            Input {
                cli_args: None,
                file_args: None
            }
        );
        let vlc_http_config = VlcHttp::try_from(vlc_http_config)?;
        let web_config = if serve.or().into_inner() {
            Some(WebServer::try_from(web_config)?)
        } else {
            web_config
                .cli_args
                .warn_if_unused()
                .map_err(UsageError::Clap)?;
            None
        };
        let cli_config = Cli::try_from(cli_config).ignore_never();
        let sequencer_config = Sequencer::try_from(sequencer_config)?;
        Ok(Self {
            vlc_http_config,
            web_config,
            cli_config,
            sequencer_config,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::ConfigFileErrorNoContext;
    use crate::config::args::RawArgs;

    #[test]
    #[allow(clippy::bool_assert_comparison)] // style, idk how to explain
    fn disallows_config_file_recursive() -> Result<(), ConfigFileErrorNoContext> {
        let config_str = "serve=true";
        let config: RawArgs = toml::from_str(config_str)?;
        assert_eq!(config.serve, true);
        assert_eq!(config.config_file, None);

        let fail_str = "foobar=\"hello.txt\"";
        let result: Result<RawArgs, _> = toml::from_str(fail_str);
        dbg!(&result);
        assert!(result.is_err());

        let fail_str = "config_file=\"hello.txt\"";
        let result: Result<RawArgs, _> = toml::from_str(fail_str);
        dbg!(&result);
        assert!(result.is_err());
        Ok(())
    }
}
