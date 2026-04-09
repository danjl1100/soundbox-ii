// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Tasks for the external `VLC` application
use std::process::Command;

/// Runs VLC with required arguments for the web interface
#[derive(Debug, clap::Args)]
pub struct RunWeb {
    input_args: Vec<String>,
}
impl RunWeb {
    /// Executes `VLC` with standard arguments for HTTP control
    ///
    /// # Errors
    /// Returns an error if the arguments are missing/invalid, or executing VLC fails
    pub fn run_web(self) -> eyre::Result<()> {
        let Self { input_args } = self;

        let args = {
            let mut builder = ArgsBuilder::from_env()?;
            builder.fill_from_args(input_args.into_iter()).transpose()?;
            builder.finish()?
        };
        run(args)
    }
}

fn run(args: Args) -> eyre::Result<()> {
    fn cmd_args(c: &mut Command, args: Args) -> &mut Command {
        let Args {
            bind_host,
            port,
            password,
        } = args;
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
    let cmd = "vlc";

    #[cfg(unix)]
    {
        // replace the current process
        crate::unix_exec::exec_cmd(cmd, |c| cmd_args(c, args)).map(|never| match never {})
    }

    #[cfg(not(unix))]
    {
        // Fallback for non-Unix systems
        crate::run_cmd(cmd, |c| cmd_args(c, args))
    }
}

#[derive(Debug)]
struct Args {
    bind_host: String,
    port: u16,
    password: String,
}
#[derive(Debug)]
struct ArgsBuilder {
    bind_host: Option<String>,
    port: Option<Result<u16, std::num::ParseIntError>>,
    password: Option<String>,
}
impl ArgsBuilder {
    fn from_env() -> eyre::Result<Self> {
        Ok(Self {
            bind_host: env_optional("VLC_BIND_HOST")?,
            port: env_optional("VLC_PORT")?.map(|s| s.parse()),
            password: env_optional("VLC_PASSWORD")?,
        })
    }
    fn fill_from_args(
        &mut self,
        mut input: impl Iterator<Item = String>,
    ) -> Option<eyre::Result<()>> {
        while let Some(arg) = input.next() {
            match &*arg {
                "--bind-host" => {
                    self.bind_host = Some(input.next()?);
                }
                "--port" => {
                    let port: Result<u16, _> = input.next()?.parse();
                    self.port = Some(port);
                }
                "--password" => {
                    self.password = Some(input.next()?);
                }
                unknown => return Some(Err(eyre::eyre!("unknown arg {unknown:?}"))),
            }
        }
        Some(Ok(()))
    }
    fn finish(self) -> eyre::Result<Args> {
        let Self {
            bind_host,
            port,
            password,
        } = self;

        let Some(bind_host) = bind_host else {
            eyre::bail!("missing arg --bind-host")
        };
        let Some(port) = port else {
            eyre::bail!("missing arg --port")
        };
        let port = port?;
        let Some(password) = password else {
            eyre::bail!("missing arg --password")
        };

        Ok(Args {
            bind_host,
            port,
            password,
        })
    }
}

fn env_optional(env: &str) -> eyre::Result<Option<String>> {
    match std::env::var(env) {
        Ok(v) => Ok(Some(v)),
        Err(e) => match e {
            std::env::VarError::NotPresent => Ok(None),
            std::env::VarError::NotUnicode(e) => {
                eyre::bail!("invalid env var value for {env:?}: {e:?}")
            }
        },
    }
}
