// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Tasks for the external `VLC` application
use std::process::Command;

/// Runs VLC with required arguments for the web interface
#[derive(Debug, clap::Args)]
pub struct RunWeb {
    #[clap(flatten)]
    args: Args,
}
impl RunWeb {
    /// Executes `VLC` with standard arguments for HTTP control
    ///
    /// # Errors
    /// Returns an error if the arguments are missing/invalid, or executing VLC fails
    pub fn run_web(self) -> eyre::Result<()> {
        let Self { args } = self;
        args.run()
    }
}

#[derive(Clone, Debug, clap::Args)]
struct Args {
    #[arg(long, env = "VLC_BIND_HOST")]
    bind_host: String,
    #[arg(long, env = "VLC_PORT")]
    port: u16,
    #[arg(long, env = "VLC_PASSWORD")]
    password: String,
}
impl Args {
    fn run(self) -> eyre::Result<()> {
        let cmd = "vlc";

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
        let Self {
            bind_host,
            port,
            password,
        } = self;
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
