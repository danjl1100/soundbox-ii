// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Parses command-line arguments

use std::borrow::Cow;
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
}
pub struct VlcHttpConfig(pub vlc_http::Authorization);
pub struct ServerConfig {
    interactive: bool,
    pub bind_address: SocketAddr,
    pub static_assets: PathBuf,
    pub watch_assets: bool,
}
impl Config {
    pub fn is_interactive(&self) -> bool {
        self.server_config
            .as_ref()
            .map_or(true, |server_config| server_config.interactive)
    }
}

impl Config {
    const SERVE_HTTP: &'static str = "serve";
}
impl VlcHttpConfig {
    const VLC_HOST: &'static str = vlc_http::auth::ENV_VLC_HOST;
    const VLC_PORT: &'static str = vlc_http::auth::ENV_VLC_PORT;
    const VLC_PASSWORD: &'static str = vlc_http::auth::ENV_VLC_PASSWORD;
}
impl ServerConfig {
    const ENV_BIND_ADDRESS: &'static str = "BIND_ADDRESS";
    const BIND_ADDRESS: &'static str = Self::ENV_BIND_ADDRESS;
    const INTERACTIVE: &'static str = "interactive";
    const STATIC_ASSETS: &'static str = "static-assets";
    const WATCH_ASSETS: &'static str = "watch-assets";
}
mod args_def {
    use super::{Config, ServerConfig, VlcHttpConfig};
    use clap::Arg;
    use std::net::SocketAddr;
    impl Config {
        pub(super) fn attach_args(app: clap::Command<'_>) -> clap::Command<'_> {
            let app = VlcHttpConfig::attach_args(app);
            let app = ServerConfig::attach_args(app);
            app.arg(
                Arg::new(Self::SERVE_HTTP)
                    .long(Self::SERVE_HTTP)
                    .help("Enables the HTTP server"),
            )
        }
    }
    impl VlcHttpConfig {
        fn attach_args(app: clap::Command<'_>) -> clap::Command<'_> {
            app.arg(
                Arg::new(Self::VLC_HOST)
                    .long(Self::VLC_HOST)
                    .takes_value(true)
                    .help("Address of VLC-HTTP server (overrides environment variable)"),
            )
            .arg(
                Arg::new(Self::VLC_PORT)
                    .long(Self::VLC_PORT)
                    .takes_value(true)
                    .help("Port of VLC-HTTP server (overrides environment variable)"),
            )
            .arg(
                Arg::new(Self::VLC_PASSWORD)
                    .long(Self::VLC_PASSWORD)
                    .takes_value(true)
                    .help("Password of VLC-HTTP server (overrides environment variable)"),
            )
        }
    }
    impl ServerConfig {
        fn attach_args(app: clap::Command<'_>) -> clap::Command<'_> {
            app.arg(
                Arg::new(Self::INTERACTIVE)
                    .short('i')
                    .long(Self::INTERACTIVE)
                    .help("Activates the command-line interface (if HTTP server disabled, defaults to enabled)"),
            )
            .arg(
                Arg::new(Self::BIND_ADDRESS)
                    .short('b')
                    .long(Self::BIND_ADDRESS)
                    // only accepts &str reference
                    //   .default_value(default_bind_address)
                    .help("Address and port to bind the HTTP server (overrides environment variable)"),
            )
            .arg(
                Arg::new(Self::STATIC_ASSETS)
                    .long(Self::STATIC_ASSETS)
                    .default_value("dist/")
                    .help("static asserts folder path (created by frontend)"),
            )
            .arg(
                Arg::new(Self::WATCH_ASSETS)
                    .long(Self::WATCH_ASSETS)
                    .short('w')
                    .help("watches the assets folder path and refreshes frontend clients when changed"),
            )
        }
        pub fn get_default_bind_address() -> String {
            std::env::var(Self::ENV_BIND_ADDRESS)
                .unwrap_or_else(|_| SocketAddr::from(([127, 0, 0, 1], 3030)).to_string())
        }
    }
}

pub fn parse_or_exit() -> Config {
    let mut command = Config::attach_args(clap::command!());
    let matches = command.get_matches_mut();

    match Config::try_from(&matches) {
        Ok(config) => config,
        Err(message) => {
            eprintln!("{}", command.render_usage());
            eprintln!();
            eprintln!("ERROR: {}", message);
            std::process::exit(1)
        }
    }
}

impl<'a> TryFrom<&'a clap::ArgMatches> for Config {
    type Error = String;
    fn try_from(matches: &clap::ArgMatches) -> Result<Self, String> {
        let server_config = if matches.is_present(Config::SERVE_HTTP) {
            Some(ServerConfig::try_from(matches)?)
        } else {
            None
        };
        let vlc_http_config = VlcHttpConfig::try_from(matches)?;
        //
        Ok(Config {
            vlc_http_config,
            server_config,
        })
    }
}

impl<'a> TryFrom<&'a clap::ArgMatches> for ServerConfig {
    type Error = String;
    fn try_from(matches: &'a clap::ArgMatches) -> Result<Self, String> {
        let bind_address = {
            let input = matches.value_of(Self::BIND_ADDRESS).map_or_else(
                || Cow::Owned(ServerConfig::get_default_bind_address()),
                Cow::Borrowed,
            );
            SocketAddr::from_str(&input)
                .map_err(|err| format!("{err} ({} argument \"{input}\")", Self::BIND_ADDRESS))?
        };
        let static_assets = matches
            .value_of(Self::STATIC_ASSETS)
            .ok_or_else(|| "missing static-assets folder".to_string())
            .and_then(|s| match PathBuf::from_str(s) {
                Ok(path) => match (path.exists(), path.is_dir()) {
                    (false, _) => Err(format!("static-assets path \"{s}\" does not exist")),
                    (_, false) => Err(format!("static-assets path \"{s}\" is not a folder")),
                    (true, true) => Ok(path),
                },
                Err(never) => match never {},
            })?;
        Ok(ServerConfig {
            interactive: matches.is_present(Self::INTERACTIVE),
            bind_address,
            static_assets,
            watch_assets: matches.is_present(Self::WATCH_ASSETS),
        })
    }
}

impl<'a> TryFrom<&'a clap::ArgMatches> for VlcHttpConfig {
    type Error = String;
    fn try_from(matches: &clap::ArgMatches) -> Result<VlcHttpConfig, String> {
        use vlc_http::auth::{Authorization, Credentials, PartialConfig};
        const NOTE_CMD_HELP: &str =
            "NOTE: View command-line help (-h) for alternate methods of specifying VLC-HTTP parameters.";
        //
        let format_err_port = |(port_str, err)| format!("invalid port \"{port_str}\" ({err})");
        let format_err_partial =
            |partial| format!("incomplete VLC-HTTP {partial}\n{NOTE_CMD_HELP}");
        let format_err_uri = |(uri, err)| format!("invalid VLC-HTTP host/port ({err} \"{uri}\")");
        let unwrap_val = |key| matches.value_of(key).map(String::from).ok_or(());
        let merge_with_env = |arg_config| {
            let env_config = PartialConfig::from_env();
            Credentials::try_from_partial(env_config.override_with(arg_config))
        };
        //
        let host = unwrap_val(Self::VLC_HOST);
        let port = unwrap_val(Self::VLC_PORT);
        let password = unwrap_val(Self::VLC_PASSWORD);
        let arg_config = PartialConfig {
            password,
            host,
            port,
        };
        let credentials = {
            let input = Credentials::try_from_partial(arg_config).or_else(merge_with_env);
            let complete = input.map_err(format_err_partial)?;
            complete.map_err(format_err_port)?
        };
        let auth = Authorization::try_from(credentials).map_err(format_err_uri)?;
        Ok(Self(auth))
    }
}
