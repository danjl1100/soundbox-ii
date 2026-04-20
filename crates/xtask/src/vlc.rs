// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Tasks for the external `VLC` application
use crate::confirm_until_yes;
use std::process::Command;
use vlc_http_auth::AuthInput;

const ARG_VLC_AUTH_FILE: &str = "--vlc-auth-file";

/// Runs VLC with required arguments for the web interface
#[derive(Debug, clap::Args)]
pub struct RunWeb {
    #[clap(flatten)]
    bind_args: vlc_http_auth_clap::ClapAuthInputOptional,
    /// TOML file containing VLC authentication
    // NOTE: keep name in sync with `ARG_VLC_AUTH_FILE`
    #[clap(long)]
    vlc_auth_file: Option<std::path::PathBuf>,
}
impl RunWeb {
    /// Executes `VLC` with standard arguments for HTTP control
    ///
    /// # Errors
    /// Returns an error if the arguments are missing/invalid, or executing VLC fails
    pub fn run_web(self) -> eyre::Result<()> {
        let Self {
            bind_args,
            vlc_auth_file,
        } = self;

        let auth = combine_vlc_auth_args_and_file(bind_args, vlc_auth_file)?;
        ArbitratedInput { auth }.run()
    }
}

#[derive(Clone, Debug)]
struct ArbitratedInput {
    auth: AuthInput,
}
impl ArbitratedInput {
    fn run(self) -> eyre::Result<()> {
        let cmd = "vlc";

        println!();
        confirm_until_yes(
            &"ACKNOWLEDGE: Once launched, you must manually click View > Add Interface > Web",
        )?;
        println!();

        #[cfg(unix)]
        {
            // replace the current process
            crate::unix_exec::exec_cmd(cmd, |c| self.cmd_args(c)).map(|never| match never {})
        }

        #[cfg(not(unix))]
        {
            // Fallback for non-Unix systems
            crate::run_cmd(cmd, |c| self.cmd_args(c))
        }
    }
    fn cmd_args(self, c: &mut Command) -> &mut Command {
        use vlc_http_auth::{Host, Password, Port};

        let Self { auth } = self;

        let vlc_http_auth::AuthInput {
            vlc_password: Password(password),
            vlc_host: Host(host),
            vlc_port: Port(port),
        } = auth;

        // NOTE: using a loose definition of "host" as "bind_host",
        // for symmetry in server vs client
        let bind_host = host;

        c.args([
            // personal setting, but let's bake it in here too
            "--audio-replay-gain-mode",
            "track",
        ])
        // required arguments
        .arg("--http-host")
        .arg(bind_host)
        .arg("--http-port")
        .arg(port.to_string())
        .arg("--http-password")
        .arg(password)
    }
}

fn combine_vlc_auth_args_and_file(
    auth_args: vlc_http_auth_clap::ClapAuthInputOptional,
    auth_file: Option<std::path::PathBuf>,
) -> eyre::Result<AuthInput> {
    //! NOTE: This could be a shared utility function, but the error message customization and
    //! specific config file type (e.g. where `vlc_auth` is a nested field) negate the benefit.

    use arg_util::ConfigOpenOrWriteTemplate as _;
    use eyre::Context as _;

    // incomplete args?
    let args_err = match AuthInput::try_from(auth_args.into_common()) {
        // skip config file if args are complete
        Ok(complete_args) => return Ok(complete_args),
        Err(e) => e,
    };
    // config file available?
    let Some(auth_file) = auth_file else {
        return Err(args_err).with_context(|| {
            format!("incomplete VLC HTTP auth args, and no {ARG_VLC_AUTH_FILE} provided")
        });
    };

    // read config file
    let auth_file = AuthInput::open_or_write_template(
        &auth_file,
        "VLC HTTP auth template file",
        AuthInput::sample_for_templates,
    )?;

    // combine args with config file
    Ok(args_err.into_inner().unwrap_or(auth_file))
}
