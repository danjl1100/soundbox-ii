// Copyright (C) 2021-2023  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use super::{env_vars, Input, SequencerError, UsageError, Value, VlcHttpError, WebError};
use clap::Parser;
use serde::Deserialize;
use std::path::PathBuf;
use std::str::FromStr;
use std::{convert::TryFrom, net::SocketAddr};

arg_util::derive_unpack! {
    #[derive(Parser, Deserialize, Default, Debug)]
    #[serde(deny_unknown_fields)]
    #[clap(version)]
    // duplicated here, as nix flake build does not capture Cargo.toml description (unclear why)
    #[clap(about = "Interactive graph-based sequencer for beets songs playing on a VLC backend.")]
    pub(super) struct RawArgs impl unpacked as RawArgsUnpacked {
        #[clap(flatten)]
        #[serde(default)]
        pub vlc_http_config: RawVlcHttp,
        #[clap(flatten)]
        #[serde(default)]
        pub web_config: RawWebServer,
        #[clap(flatten)]
        #[serde(default)]
        pub cli_config: RawCli,
        #[clap(flatten)]
        #[serde(default)]
        pub sequencer_config: RawSequencer,
        /// Enables the HTTP server
        #[clap(long)]
        #[serde(default)]
        pub serve: bool,
        /// Load configuration from the specified file
        #[clap(long = env_vars::CONFIG_FILE)]
        #[serde(skip)]
        pub config_file: Option<PathBuf>,
    }
    #[derive(Parser, Deserialize, Default, Debug)]
    pub(super) struct RawVlcHttp impl unpacked as RawVlcHttpUnpacked {
        /// Password of VLC-HTTP server (overrides environment variable)
        #[clap(long = env_vars::VLC_PASSWORD)]
        vlc_password: Option<String>,
        /// Address of VLC-HTTP server (overrides environment variable)
        #[clap(long = env_vars::VLC_HOST)]
        vlc_host: Option<String>,
        /// Port of VLC-HTTP server (overrides environment variable)
        #[clap(long = env_vars::VLC_PORT)]
        vlc_port: Option<u16>,
    }
    #[derive(Parser, Deserialize, Default, Debug)]
    pub(super) struct RawWebServer impl unpacked as RawWebServerUnpacked {
        /// Address and port to bind the HTTP server (overrides environment variable)
        #[clap(short = 'b', long = env_vars::BIND_ADDRESS)]
        bind_address: Option<String>,
        /// static asserts folder path (created by frontend, overrides environment variable)
        #[clap(long = env_vars::STATIC_ASSETS)]
        static_assets: Option<String>,
        /// watches the assets folder path and refreshes frontend clients when changed
        #[clap(short = 'w', long = env_vars::WATCH_ASSETS)]
        watch_assets: bool,
    }
    #[derive(Parser, Deserialize, Default, Debug)]
    pub(super) struct RawCli impl unpacked as RawCliUnpacked {
        /// Activates the command-line interface (if HTTP server disabled, defaults to enabled)
        #[clap(short = 'i', long)]
        interactive: bool,
        /// Script file to run
        ///
        /// End script with the quit command to exit after running
        run_script: Option<PathBuf>,
        /// File to load state, then periodically store
        #[clap(long = env_vars::STATE_FILE)]
        state_file: Option<PathBuf>,
    }
    #[derive(Parser, Deserialize, Default, Debug)]
    pub(super) struct RawSequencer impl unpacked as RawSequencerUnpacked {
        /// Root folder for querying `FileLines` or `FolderListing` sources (overrides environment variable)
        #[clap(long = env_vars::ROOT_FOLDER)]
        root_folder: Option<String>,
        /// Executable to run for querying `Beet` sources (overrides environment variable)
        #[clap(long = env_vars::BEET_CMD)]
        beet_cmd: Option<String>,
    }
}

impl TryFrom<Input<RawVlcHttp>> for super::VlcHttp {
    type Error = super::Error;

    fn try_from(raw: Input<RawVlcHttp>) -> Result<Self, Self::Error> {
        use vlc_http::auth::{Authorization, Credentials};
        let parse_port = |key, value| {
            vlc_http::auth::Credentials::parse_port(value).map_err(|(value, err)| UsageError::Env {
                key,
                message: format!("invalid number {value}: {err}"),
            })
        };
        let RawVlcHttpUnpacked {
            vlc_password,
            vlc_host,
            vlc_port,
        } = raw.into();
        let vlc_password = vlc_password.env(env_vars::VLC_PASSWORD).get_first_str();
        let vlc_host = vlc_host.env(env_vars::VLC_HOST).get_first_str();
        let vlc_port = vlc_port.env(env_vars::VLC_PORT).try_get_first(parse_port)?;
        let (password, host, port) = match (vlc_password, vlc_host, vlc_port) {
            (Some(password), Some(host), Some(port)) => Ok((password, host, port)),
            (password @ None, host, port)
            | (password, host @ None, port)
            | (password, host, port @ None) => Err(VlcHttpError::Incomplete {
                password: password.map(Value::into_inner),
                host: host.map(Value::into_inner),
                port: port.map(Value::into_inner),
            }),
        }?;
        let Value(password, password_source) = password;
        let Value(host, host_source) = host;
        let Value(port, port_source) = port;
        let credentials = Credentials {
            password,
            host,
            port,
        };
        let auth = Authorization::try_from(credentials).map_err(|(url, error)| {
            VlcHttpError::InvalidUri {
                url,
                error,
                host_source,
                port_source,
            }
        })?;
        Ok(Self(auth))
    }
}

impl RawWebServer {
    pub fn warn_if_unused(self) -> Result<(), String> {
        let Self {
            bind_address,
            static_assets,
            watch_assets,
        } = self;
        {
            // WARNINGS - print, but continue
            let warn_inactive = |name: &str| {
                println!("WARNING: `{name}` is ignored when `serve` is not enabled");
            };
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

impl TryFrom<Input<RawWebServer>> for super::WebServer {
    type Error = WebError;

    fn try_from(raw: Input<RawWebServer>) -> Result<Self, Self::Error> {
        const DEFAULT_BIND_ADDRESS: ([u8; 4], u16) = ([127, 0, 0, 1], 3030);
        const DEFAULT_STATIC_ASSETS: Value<&str> = Value::define_default("dist/");
        let RawWebServerUnpacked {
            bind_address,
            static_assets,
            watch_assets,
        } = raw.into();
        let bind_address = {
            let input = bind_address.env(env_vars::BIND_ADDRESS).get_first_str();
            match input {
                Some(address) => SocketAddr::from_str(address.inner())
                    .map_err(|error| WebError::BindAddress { address, error })?,
                None => SocketAddr::from(DEFAULT_BIND_ADDRESS),
            }
        };
        let static_assets = {
            let folder = static_assets
                .env(env_vars::STATIC_ASSETS)
                .get_first_str()
                .unwrap_or_else(|| DEFAULT_STATIC_ASSETS.map(str::to_string));
            let folder_path = PathBuf::from_str(folder.inner()).unwrap_or_else(|n| match n {});
            // -------------------------------------------------------------------------------
            // TODO migrate types to common arg_util crate for `CheckedFolder` and `CheckedFile` to encode that they recently existed
            // 1. Move `sequencer::sources::RootFolder` to arg_util as CheckedFolder
            // 2. Move `sequencer::sources::beet::BeetCommand` to arg_util as CheckedFile
            // 3. Change all types in `Config` to use the Checked* types for all files
            // -------------------------------------------------------------------------------
            sequencer::sources::RootFolder::check_to_inner(folder_path)
                .map_err(|error| WebError::StaticAssets { folder, error })?
        };
        let watch_assets = watch_assets
            .env(env_vars::WATCH_ASSETS)
            .or_parse_bool()
            .into_inner();
        Ok(Self {
            bind_address,
            static_assets,
            watch_assets,
        })
    }
}

impl TryFrom<Input<RawCli>> for super::Cli {
    type Error = shared::Never;

    fn try_from(raw: Input<RawCli>) -> Result<Self, Self::Error> {
        let RawCliUnpacked {
            interactive,
            run_script,
            state_file,
        } = raw.into();
        let force_interactive = interactive.or().into_inner();
        let run_script = run_script.get_first();
        let state_file = state_file.get_first();
        Ok(Self {
            force_interactive,
            // run_script,
            // state_file,
        })
    }
}

impl TryFrom<Input<RawSequencer>> for super::Sequencer {
    type Error = SequencerError;

    fn try_from(raw: Input<RawSequencer>) -> Result<Self, Self::Error> {
        const DEFAULT_ROOT_FOLDER: Value<&str> = Value::define_default(".");
        const DEFAULT_BEET_CMD: Value<&str> = Value::define_default("beet");
        let RawSequencerUnpacked {
            root_folder,
            beet_cmd,
        } = raw.into();
        let root_folder = {
            let folder = root_folder
                .env(env_vars::ROOT_FOLDER)
                .get_first_str()
                // TODO consider using Cow, then Cow::ToOwned if needed for error type
                // (to avoid allocating for the Default case)
                .unwrap_or_else(|| DEFAULT_ROOT_FOLDER.map(str::to_string));
            let folder_path =
                PathBuf::from_str(folder.inner()).unwrap_or_else(|never| match never {});
            sequencer::sources::RootFolder::new(folder_path)
                .map_err(|error| SequencerError::RootFolder { folder, error })?
        };
        // TODO should this be optional?
        // if so, would error immediately when attempting to create a beet filter (load file, or user driven)
        let beet_cmd = {
            let Value(cmd, source) = beet_cmd
                .env(env_vars::BEET_CMD)
                .get_first_str()
                // TODO consider using Cow, see note above
                .unwrap_or_else(|| DEFAULT_BEET_CMD.map(str::to_string));
            sequencer::sources::Beet::new(cmd)
                .map_err(|error| SequencerError::BeetCommand { source, error })?
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
