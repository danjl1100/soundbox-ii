//! Parses command-line arguments

use std::borrow::Cow;
use std::convert::TryFrom;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;

pub struct Config {
    pub vlc_http_config: VlcHttpConfig,
    /// Configuration for the webserver.
    /// If `None`, then interactive mode implicitly enabled
    pub server_config: Option<ServerConfig>,
}
pub struct VlcHttpConfig(pub vlc_http::Credentials);
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
    const DISABLE_SERVER: &'static str = "disable-server";
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
                Arg::new(Self::DISABLE_SERVER)
                    .long(Self::DISABLE_SERVER)
                    .help("Disables the HTTP server"),
            )
        }
    }
    impl VlcHttpConfig {
        fn attach_args(app: clap::Command<'_>) -> clap::Command<'_> {
            app.arg(
                Arg::new(VlcHttpConfig::VLC_HOST)
                    .long(VlcHttpConfig::VLC_HOST)
                    .takes_value(true)
                    .help("Address of VLC-HTTP server (overrides environment variable)"),
            )
            .arg(
                Arg::new(VlcHttpConfig::VLC_PORT)
                    .long(VlcHttpConfig::VLC_PORT)
                    .takes_value(true)
                    .help("Port of VLC-HTTP server (overrides environment variable)"),
            )
            .arg(
                Arg::new(VlcHttpConfig::VLC_PASSWORD)
                    .long(VlcHttpConfig::VLC_PASSWORD)
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
                    .long(ServerConfig::INTERACTIVE)
                    .help("Activates the command-line interface"),
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
                    .short('s')
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
            std::env::var(ServerConfig::ENV_BIND_ADDRESS)
                .unwrap_or_else(|_| SocketAddr::from(([127, 0, 0, 1], 3030)).to_string())
        }
    }
}

pub fn parse_or_exit() -> Config {
    let mut command = Config::attach_args(clap::command!());
    let matches = command.get_matches_mut();

    match build_config(&matches) {
        Ok(config) => config,
        Err(message) => {
            eprintln!("{}", command.render_usage());
            eprintln!();
            eprintln!("ERROR: {}", message);
            std::process::exit(1)
        }
    }
}

impl<'a> TryFrom<&'a clap::ArgMatches> for ServerConfig {
    type Error = String;
    fn try_from(matches: &'a clap::ArgMatches) -> Result<Self, String> {
        let bind_address = matches.value_of(Self::BIND_ADDRESS).map_or_else(
            || Cow::Owned(ServerConfig::get_default_bind_address()),
            Cow::Borrowed,
        );
        let bind_address = SocketAddr::from_str(&bind_address).map_err(|err| {
            format!(
                "{} ({} argument \"{}\")",
                err,
                Self::BIND_ADDRESS,
                bind_address
            )
        })?;
        let static_assets = matches
            .value_of(Self::STATIC_ASSETS)
            .ok_or_else(|| "missing static-assets folder".to_string())
            .and_then(|s| match PathBuf::from_str(s) {
                Err(err) => Err(format!(
                    "{} ({} argument \"{}\")",
                    err,
                    Self::STATIC_ASSETS,
                    s
                )),
                Ok(path) => match (path.exists(), path.is_dir()) {
                    (false, _) => Err(format!("static-assets path \"{}\" does not exist", s)),
                    (_, false) => Err(format!("static-assets path \"{}\" is not a folder", s)),
                    (true, true) => Ok(path),
                },
            })?;
        Ok(ServerConfig {
            interactive: matches.is_present(Self::INTERACTIVE),
            bind_address,
            static_assets,
            watch_assets: matches.is_present(Self::WATCH_ASSETS),
        })
    }
}

fn build_config(matches: &clap::ArgMatches) -> Result<Config, String> {
    let server_config = if matches.is_present(Config::DISABLE_SERVER) {
        None
    } else {
        Some(ServerConfig::try_from(matches)?)
    };
    let vlc_http_config = VlcHttpConfig::try_from(matches)?;
    //
    Ok(Config {
        vlc_http_config,
        server_config,
    })
}
impl<'a> TryFrom<&'a clap::ArgMatches> for VlcHttpConfig {
    type Error = String;
    fn try_from(matches: &clap::ArgMatches) -> Result<VlcHttpConfig, String> {
        use vlc_http::auth::{Config, Credentials, PartialConfig};
        const NOTE_CMD_HELP: &str =
            "NOTE: View command-line help (-h) for alternate methods of specifying VLC-HTTP parameters.";
        //
        let format_err_port = |(port_str, err)| format!("invalid port \"{}\" ({})", port_str, err);
        let format_err_partial =
            |partial| format!("incomplete VLC-HTTP {}\n{}", partial, NOTE_CMD_HELP);
        let format_err_uri =
            |(uri, uri_err)| format!("invalid VLC-HTTP host/port ({} \"{}\")", uri_err, uri);
        let unwrap_val = |key| matches.value_of(key).map(String::from).ok_or(());
        //
        let host = unwrap_val(Self::VLC_HOST);
        let port = unwrap_val(Self::VLC_PORT);
        let password = unwrap_val(Self::VLC_PASSWORD);
        let arg_config = PartialConfig {
            password,
            host,
            port,
        };
        let merge_with_env = |arg_config| {
            let env_config = PartialConfig::from_env();
            Config::try_from_partial(env_config.override_with(arg_config))
        };
        let config = Config::try_from_partial(arg_config)
            .or_else(merge_with_env)
            .map(|result| result.map_err(format_err_port))
            .map_err(format_err_partial)??;
        Credentials::try_from(config)
            .map(VlcHttpConfig)
            .map_err(format_err_uri)
    }
}
