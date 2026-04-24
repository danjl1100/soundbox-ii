// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Input authentication parameters to the VLC instance using [`clap`]

pub use self::file::{ARG_VLC_AUTH_FILE, ClapAuthFile};
pub use self::input::ClapAuthInput;
pub use self::input_optional::ClapAuthInputOptional;
pub use self::merge::ClapAuthInputAndFile;

pub use ::clap as clap_crate;

mod input {
    use vlc_http_auth::{AuthInput, Host, Password, Port};

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
}

mod input_optional {
    use vlc_http_auth::{Host, Password, Port, optional::AuthInputOptional};

    /// Optional parts from [`clap`] inputs, to be combined
    /// to form a whole [`AuthInput`]
    ///
    /// [`AuthInput`]: `vlc_http_auth::AuthInput`
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
        ///
        /// [`ClapAuthInput`]: `crate::ClapAuthInput`
        /// [`AuthInput`]: `vlc_http_auth::AuthInput`
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
}

mod file {
    /// User-friendly name for the argument specified in [`ClapAuthFile`]
    pub const ARG_VLC_AUTH_FILE: &str = "--vlc-auth-file";

    /// Convenience for standard naming of `--vlc-auth-file` (env `VLC_AUTH_FILE`)
    #[derive(Clone, clap::Args, Debug)]
    pub struct ClapAuthFile {
        /// TOML file containing VLC authentication
        // NOTE: keep name in sync with `ARG_VLC_AUTH_FILE`
        #[clap(long, env = "VLC_AUTH_FILE")]
        vlc_auth_file: Option<std::path::PathBuf>,
    }
    impl ClapAuthFile {
        /// Returns the inner optional path
        #[must_use]
        pub fn into_inner(self) -> Option<std::path::PathBuf> {
            let Self { vlc_auth_file } = self;
            vlc_auth_file
        }
    }
}

mod merge {
    use crate::{ARG_VLC_AUTH_FILE, ClapAuthFile, ClapAuthInputOptional};
    use arg_util::config_file::ErrorOpenWrite;
    use vlc_http_auth::{AuthInput, optional::MissingPartsError};

    /// Both [`ClapAuthInputOptional`] and [`ClapAuthFile`] for convenient use in
    /// [`Self::merge`]
    #[derive(Clone, clap::Args, Debug)]
    pub struct ClapAuthInputAndFile {
        #[clap(flatten)]
        input: ClapAuthInputOptional,
        #[clap(flatten)]
        file: ClapAuthFile,
    }
    impl From<(ClapAuthInputOptional, ClapAuthFile)> for ClapAuthInputAndFile {
        fn from(value: (ClapAuthInputOptional, ClapAuthFile)) -> Self {
            let (input, file) = value;
            ClapAuthInputAndFile { input, file }
        }
    }

    impl ClapAuthInputAndFile {
        /// Combines with the arguments with the config file (if needed)
        ///
        /// # Errors
        /// Returns an error if no file is provided (with incomplete args), or the open function
        /// fails
        pub fn merge(self) -> Result<AuthInput, Error> {
            use arg_util::ConfigOpenOrWriteTemplate as _;

            let Self { input, file } = self;

            let make_err = |kind| Error { kind };

            // incomplete args?
            let args_err = match AuthInput::try_from(input.into_common()) {
                // skip config file if args are complete
                Ok(complete_args) => return Ok(complete_args),
                Err(e) => e,
            };
            // config file available?
            let Some(auth_file) = file.into_inner() else {
                return Err(make_err(ErrorKind::IncompleteNoFile(args_err)));
            };

            // read config file

            let auth_file = AuthInput::open_or_write_template(
                &auth_file,
                "VLC HTTP auth template file",
                AuthInput::sample_for_templates,
            )
            .map_err(ErrorKind::ErrorOpen)
            .map_err(make_err)?;

            // combine args with config file
            Ok(args_err.into_inner().unwrap_or(auth_file))
        }
    }

    #[derive(Debug)]
    pub struct Error {
        kind: ErrorKind,
    }
    #[derive(Debug)]
    enum ErrorKind {
        IncompleteNoFile(MissingPartsError),
        ErrorOpen(ErrorOpenWrite),
    }
    impl std::error::Error for Error {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            match &self.kind {
                ErrorKind::IncompleteNoFile(source) => Some(source),
                ErrorKind::ErrorOpen(transparent) => transparent.source(),
            }
        }
    }
    impl std::fmt::Display for Error {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let Self { kind } = self;
            match kind {
                ErrorKind::IncompleteNoFile(_) => write!(
                    f,
                    "incomplete VLC HTTP auth args, and no {ARG_VLC_AUTH_FILE} provided"
                ),
                ErrorKind::ErrorOpen(transparent) => write!(f, "{transparent}"),
            }
        }
    }
}
