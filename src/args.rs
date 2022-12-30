// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Parses command-line arguments

use clap::Parser;
use std::convert::TryFrom;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;

pub struct Config {
    /// Configuration for the VLC HTTP interface
    pub vlc_http_config: VlcHttpConfig,
    /// Configuration for the webserver
    /// If `None`, then interactive mode implicitly enabled
    pub server_config: Option<ServerConfig>,
    /// Configuration for the Sequencer item source(s)
    pub sequencer_config: SequencerConfig,
}
pub struct VlcHttpConfig(pub vlc_http::Authorization);
pub struct ServerConfig {
    interactive: bool,
    pub bind_address: SocketAddr,
    pub static_assets: PathBuf,
    pub watch_assets: bool,
}
pub struct SequencerConfig {
    pub root_folder: sequencer::sources::RootFolder,
    pub beet_cmd: sequencer::sources::Beet,
}
impl Config {
    pub fn is_interactive(&self) -> bool {
        self.server_config
            .as_ref()
            .map_or(true, |server_config| server_config.interactive)
    }
}

#[derive(Parser)]
#[clap(version)]
// duplicated here, as nix flake build does not capture Cargo.toml description (unclear why)
#[clap(about = "Interactive graph-based sequencer for beets songs playing on a VLC backend.")]
struct RawArgs {
    #[clap(flatten)]
    vlc_http_config: RawVlcHttpConfig,
    #[clap(flatten)]
    server_config: RawServerConfig,
    #[clap(flatten)]
    sequencer_config: RawSequencerConfig,
    /// Enables the HTTP server
    #[clap(long)]
    serve: bool,
}
#[derive(Parser)]
struct RawVlcHttpConfig {
    /// Address of VLC-HTTP server (overrides environment variable)
    #[clap(long = vlc_http::auth::ENV_VLC_HOST)]
    vlc_host: Option<String>,
    /// Port of VLC-HTTP server (overrides environment variable)
    #[clap(long = vlc_http::auth::ENV_VLC_PORT)]
    vlc_port: Option<String>,
    /// Password of VLC-HTTP server (overrides environment variable)
    #[clap(long = vlc_http::auth::ENV_VLC_PASSWORD)]
    vlc_password: Option<String>,
}
#[derive(Parser)]
struct RawServerConfig {
    /// Activates the command-line interface (if HTTP server disabled, defaults to enabled)
    #[clap(short = 'i', long)]
    interactive: bool,
    /// Address and port to bind the HTTP server (overrides environment variable)
    #[clap(short = 'b', long = env_vars::BIND_ADDRESS)]
    bind_address: Option<String>,
    /// static asserts folder path (created by frontend, overrides environment variable)
    #[clap(long, value_name = env_vars::STATIC_ASSETS)]
    static_assets: Option<String>,
    /// watches the assets folder path and refreshes frontend clients when changed
    #[clap(short = 'w', long)]
    watch_assets: bool,
}
#[derive(Parser)]
struct RawSequencerConfig {
    /// Root folder for querying `FileLines` or `FolderListing` sources (overrides environment variable)
    #[clap(long = env_vars::ROOT_FOLDER)]
    root_folder: Option<String>,
    /// Executable to run for querying `Beet` sources (overrides environment variable)
    #[clap(long = env_vars::BEET_CMD)]
    beet_cmd: Option<String>,
}
mod env_vars {
    use super::{SequencerConfig, ServerConfig};

    pub(super) const BIND_ADDRESS: &str = "BIND_ADDRESS";
    pub(super) const BEET_CMD: &str = "BEET_CMD";
    pub(super) const ROOT_FOLDER: &str = "ROOT_FOLDER";
    pub(super) const STATIC_ASSETS: &str = "STATIC_ASSETS";
    impl ServerConfig {
        pub fn env_or_default_bind_address() -> String {
            use std::net::SocketAddr;
            std::env::var(BIND_ADDRESS)
                .unwrap_or_else(|_| SocketAddr::from(([127, 0, 0, 1], 3030)).to_string())
        }
        pub fn env_or_default_static_assets() -> String {
            std::env::var(STATIC_ASSETS).unwrap_or_else(|_| "dist/".to_string())
        }
    }
    impl SequencerConfig {
        pub fn env_or_default_beet_cmd() -> String {
            std::env::var(BEET_CMD).unwrap_or_else(|_| "beet".to_string())
        }
        pub fn env_or_default_root_folder() -> String {
            std::env::var(ROOT_FOLDER).unwrap_or_else(|_| ".".to_string())
        }
    }
}

pub fn parse_or_exit() -> Config {
    use clap::CommandFactory;
    let raw_args = RawArgs::parse();

    match Config::try_from(raw_args) {
        Ok(config) => config,
        Err(message) => {
            let usage = RawArgs::command().render_usage();
            eprintln!("{usage}");
            eprintln!();
            eprintln!("ERROR: {message}");
            std::process::exit(1)
        }
    }
}

impl TryFrom<RawArgs> for Config {
    type Error = String;

    fn try_from(raw_args: RawArgs) -> Result<Self, Self::Error> {
        let RawArgs {
            vlc_http_config,
            server_config,
            sequencer_config,
            serve,
        } = raw_args;
        let server_config = if serve {
            Some(ServerConfig::try_from(server_config)?)
        } else {
            server_config.warn_if_unused()?;
            None
        };
        let vlc_http_config = VlcHttpConfig::try_from(vlc_http_config)?;
        let sequencer_config = SequencerConfig::try_from(sequencer_config)?;
        Ok(Self {
            vlc_http_config,
            server_config,
            sequencer_config,
        })
    }
}
impl RawServerConfig {
    fn warn_if_unused(self) -> Result<(), String> {
        let Self {
            interactive,
            bind_address,
            static_assets,
            watch_assets,
        } = self;
        {
            // WARNINGS - print, but continue
            let warn_inactive =
                |name: &str| println!("WARNING: `{name}` is ignored when `serve` is not enabled");
            if interactive {
                warn_inactive("interactive");
            }
            if watch_assets {
                warn_inactive("watch-assets");
            }
        }
        {
            // ERRORS
            let not_allowed =
                |name: &str| format!("`{name}` is not allowed when `serve` is not enabled");
            if bind_address.is_some() {
                Err(not_allowed("BIND_ADDRESS"))
            } else if static_assets.is_some() {
                Err(not_allowed("static-assets"))
            } else {
                Ok(())
            }
        }
    }
}

impl TryFrom<RawServerConfig> for ServerConfig {
    type Error = String;

    fn try_from(raw: RawServerConfig) -> Result<Self, Self::Error> {
        let RawServerConfig {
            interactive,
            bind_address,
            static_assets,
            watch_assets,
        } = raw;
        let bind_address = {
            let input = bind_address.unwrap_or_else(Self::env_or_default_bind_address);
            SocketAddr::from_str(&input)
                .map_err(|err| format!("{err} (bind address argument \"{input}\")"))?
        };
        let static_assets = {
            let folder_str = static_assets.unwrap_or_else(Self::env_or_default_static_assets);
            let folder_path = PathBuf::from_str(&folder_str).unwrap_or_else(|never| match never {});
            sequencer::sources::RootFolder::check_to_inner(folder_path)
                .map_err(|err| format!("static-assets path \"{folder_str}\": {err}"))?
        };
        Ok(Self {
            interactive,
            bind_address,
            static_assets,
            watch_assets,
        })
    }
}

impl TryFrom<RawVlcHttpConfig> for VlcHttpConfig {
    type Error = String;

    fn try_from(raw: RawVlcHttpConfig) -> Result<Self, Self::Error> {
        use vlc_http::auth::{Authorization, Credentials, PartialConfig};
        const NOTE_CMD_HELP: &str =
            "NOTE: View command-line help (-h) for alternate methods of specifying VLC-HTTP parameters.";
        let RawVlcHttpConfig {
            vlc_host,
            vlc_port,
            vlc_password,
        } = raw;
        let partial = PartialConfig {
            password: vlc_password.ok_or(()),
            host: vlc_host.ok_or(()),
            port: vlc_port.ok_or(()),
        };
        let credentials = {
            let complete = Credentials::try_from_partial(partial)
                .or_else(|partial| {
                    Credentials::try_from_partial(PartialConfig::from_env().override_with(partial))
                })
                .map_err(|partial| format!("incomplete VLC-HTTP {partial}\n{NOTE_CMD_HELP}"))?;
            complete.map_err(|(port_str, err)| format!("invalid port \"{port_str}\" ({err})"))?
        };
        let auth = Authorization::try_from(credentials)
            .map_err(|(url, err)| format!("invalid VLC-HTTP host/port ({err} \"{url}\")"))?;
        Ok(Self(auth))
    }
}

impl TryFrom<RawSequencerConfig> for SequencerConfig {
    type Error = String;

    fn try_from(raw: RawSequencerConfig) -> Result<Self, Self::Error> {
        let RawSequencerConfig {
            root_folder: root_folder_opt,
            beet_cmd: beet_cmd_opt,
        } = raw;
        let root_folder = {
            let folder_str = root_folder_opt.unwrap_or_else(Self::env_or_default_root_folder);
            let folder_path = PathBuf::from_str(&folder_str).unwrap_or_else(|never| match never {});
            sequencer::sources::RootFolder::new(folder_path)
                .map_err(|err| format!("root-folder path \"{folder_str}\": {err}"))?
        };
        let beet_cmd = {
            let cmd = beet_cmd_opt.unwrap_or_else(Self::env_or_default_beet_cmd);
            sequencer::sources::Beet::new(cmd)
                .map_err(|err| format!("{} {err}", env_vars::BEET_CMD))?
        };
        Ok(Self {
            root_folder,
            beet_cmd,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::RawArgs;

    #[test]
    fn cli_args() {
        use clap::CommandFactory;
        RawArgs::command().debug_assert();
    }
}
