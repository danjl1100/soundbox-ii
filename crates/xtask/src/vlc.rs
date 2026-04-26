// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Tasks for the external `VLC` application
use crate::confirm_until_yes;
use std::process::Command;
use vlc_http_auth::AuthInput;

/// Runs VLC with required arguments for the web interface
#[derive(Debug, clap::Args)]
pub struct RunWeb {
    #[clap(flatten)]
    bind_args_and_file: vlc_http_auth_clap::ClapAuthInputAndFile,
}
impl RunWeb {
    /// Executes `VLC` with standard arguments for HTTP control
    ///
    /// # Errors
    /// Returns an error if the arguments are missing/invalid, or executing VLC fails
    pub fn run_web(self) -> eyre::Result<()> {
        let Self { bind_args_and_file } = self;

        let auth = bind_args_and_file.merge()?;
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

        cfg_select! {
            unix => {
                // replace the current process
                crate::unix_exec::exec_cmd(cmd, |c| self.cmd_args(c)).map(|never| match never {})
            }
            _ => {
                // Fallback for non-Unix systems
                crate::run_cmd(cmd, |c| self.cmd_args(c))
            }
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
