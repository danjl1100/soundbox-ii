// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use crate::BeetItem;
use std::ffi::OsStr;
use std::io::BufRead as _;
use std::{borrow::Cow, process::Command};
use tracing::{debug, trace};

/// Abstraction point, with default of [`BeetCommand`]
pub trait BeetRunner: std::fmt::Debug {
    /// Error executing the beet command
    type Error;
    /// Executes the beet command with the specified arguments and returns the output
    ///
    /// # Errors
    /// Returns an error if spawning or waiting for the command fails
    fn run_beet_command<S>(
        &mut self,
        args: impl Iterator<Item = S>,
    ) -> Result<std::process::Output, Self::Error>
    where
        S: AsRef<str>;
}

/// Executes the `beet` command from the system path
#[derive(Debug)]
pub struct BeetCommand<'a> {
    cmd_name: std::borrow::Cow<'a, OsStr>,
}
impl BeetCommand<'static> {
    /// Creates a command runner for the default `"beet"` executable name
    ///
    /// # Errors
    /// Returns an error if the specified `cmd_name` is not available for reading
    pub fn new_beet() -> Result<Self, InvalidCmdError> {
        Self::new(Cow::Borrowed("beet".as_ref()))
    }
}
impl<'a> BeetCommand<'a> {
    /// Creates a command runner with the specified path to the `beet` executable
    ///
    /// # Errors
    /// Returns an error if the specified `cmd_name` is not available for reading
    pub fn new(cmd_name: std::borrow::Cow<'a, OsStr>) -> Result<Self, InvalidCmdError> {
        use std::process::{Command, Stdio};

        let make_err = |source| InvalidCmdError {
            cmd: cmd_name.to_string_lossy().to_string(),
            source,
        };

        let mut cmd = Command::new(&cmd_name);
        cmd.arg("--help")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .stdin(Stdio::null());

        tracing::trace!(?cmd, "checking beet command executes...");
        let mut spawned = cmd.spawn().map_err(make_err)?;

        spawned.kill().map_err(make_err)?;
        spawned.wait().map_err(make_err)?;

        Ok(Self { cmd_name })
    }
}
impl BeetRunner for BeetCommand<'_> {
    type Error = std::io::Error;

    fn run_beet_command<S>(
        &mut self,
        args: impl Iterator<Item = S>,
    ) -> Result<std::process::Output, Self::Error>
    where
        S: AsRef<str>,
    {
        let Self { cmd_name } = self;

        let mut command = Command::new(cmd_name);
        for arg in args {
            command.arg(arg.as_ref());
        }

        trace!(?command);

        command.output()
    }
}

impl BeetItem {
    /// Lists the resulting items for a beet query
    ///
    /// # Errors
    /// Returns an error if the `beet` command fails or produces invalid output
    pub fn list_from_beet_query<T: BeetRunner>(
        runner: &mut T,
        filters: impl Iterator<Item = String>,
    ) -> Result<Vec<Self>, Error<T::Error>> {
        let make_error = |kind| Error { kind };

        debug!("spawn `beet` command");

        let output = runner.run_beet_command(
            ["ls", "-f$id=$path"]
                .into_iter()
                .map(std::borrow::Cow::Borrowed)
                .chain(filters.map(std::borrow::Cow::Owned)),
        );

        let output = output.map_err(ErrorKind::Spawn).map_err(make_error)?;

        if !output.stderr.is_empty() {
            let stderr_str = String::from_utf8_lossy(&output.stderr).to_string();
            tracing::trace!(stderr=?stderr_str, "beet query failed");

            return Err(make_error(ErrorKind::Stderr { stderr_str }));
        }

        if !output.status.success() {
            return Err(make_error(ErrorKind::ExitFail {
                code: output.status.code(),
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            }));
        }

        debug!("parse `beet` output ({} bytes)", output.stdout.len());

        // let output_len = output.stdout.len();
        // let output_trim = String::from_utf8_lossy(if output_len > 100 {
        //     &output.stdout[0..100]
        // } else {
        //     &output.stdout[..]
        // });
        // trace!(?output_trim, ?output_len);

        output
            .stdout
            .lines()
            .map(|line| {
                let line = line.map_err(ErrorKind::Read)?;
                line.parse()
                    .map_err(|error| ErrorKind::InvalidLine { line, error })
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(make_error)
    }
}

#[derive(Debug, thiserror::Error)]
#[error("invalid beet command {cmd:?}")]
pub struct InvalidCmdError {
    cmd: String,
    source: std::io::Error,
}

#[derive(Debug)]
pub struct Error<T> {
    kind: ErrorKind<T>,
}
#[derive(Debug)]
enum ErrorKind<T> {
    Spawn(T),
    Read(std::io::Error),
    Stderr {
        stderr_str: String,
    },
    ExitFail {
        code: Option<i32>,
        stdout: String,
    },
    InvalidLine {
        line: String,
        error: super::item::Error,
    },
}
impl<T> std::error::Error for Error<T>
where
    T: std::error::Error + 'static,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        use ErrorKind as E;
        match &self.kind {
            E::Spawn(error) => Some(error),
            E::Read(error) => Some(error),
            E::Stderr { stderr_str: _ } | E::ExitFail { .. } => None,
            E::InvalidLine { error, .. } => Some(error),
        }
    }
}
impl<T> std::fmt::Display for Error<T>
where
    T: std::error::Error,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        #[rustfmt::skip]
        fn funnel<'a, F: Fn(usize, &'a str) -> Cow<'a, str>>(f: F) -> F { f }
        let max_len = funnel(|max, raw_input: &str| {
            if raw_input.len() > max {
                Cow::Owned(format!("{} ...", &raw_input[..max]))
            } else {
                Cow::Borrowed(raw_input)
            }
        });
        let (description, details) = match &self.kind {
            ErrorKind::Spawn(_) => ("failed to spawn", None),
            ErrorKind::Read(_) => ("failed to read from", None),
            ErrorKind::Stderr { stderr_str } => {
                ("stderr output from", Some(Cow::Borrowed(&**stderr_str)))
            }
            ErrorKind::ExitFail { code, stdout } => {
                let stdout = max_len(200, stdout);
                let details = match code {
                    Some(code) => Cow::Owned(format!("[code {code}] {stdout}")),
                    None => stdout,
                };
                ("failure status code from", Some(details))
            }
            ErrorKind::InvalidLine { line, error: _ } => {
                let line = max_len(80, line);
                ("invalid output line from", Some(line))
            }
        };
        write!(f, "{description} beet command")?;
        if let Some(details) = details {
            write!(f, ": {details:?}")?;
        }
        Ok(())
    }
}
