// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Logic for the `xtask` functionality

use self::run_cmd::run_cmd;
use self::status_cmd::status_cmd;
pub use self::status_cmd::{SpawnError, SpawnFail};
pub use self::typed_err::{TypedErr, TypedResult};
use std::{
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
};

pub mod copyright;
pub mod rust;
pub mod spigot_visual;
pub mod vlc;

#[cfg(unix)]
pub mod unix_exec;

/// If present, attempt to fix the checks by writing to files (otherwise, run read-only checks)
#[derive(Clone, Copy, Debug)]
pub struct Fix;

/// If present, write output files (otherwise, run read-only checks)
#[derive(Clone, Copy, Debug)]
pub struct WriteOutput;

mod typed_err {
    use super::SpawnFail;

    /// Adapter for [`eyre::bail!`] that adapts the error type (e.g. into [`TypedErr`])
    #[macro_export]
    macro_rules! bail {
        ($($arg:tt)*) => {
            return Err(eyre::eyre!($($arg)*).into())
        };
    }

    /// Result with distinct [`SpawnFail`] case
    pub type TypedResult<T> = Result<T, TypedErr>;

    /// Typed error for [`SpawnFail`] and others
    pub struct TypedErr(TypedErrKind);
    enum TypedErrKind {
        /// Error spawning a command
        Spawn(SpawnFail),
        /// Other errors
        Eyre(eyre::Error),
    }
    impl TypedErr {
        // NOTE: [`eyre::Context`] requires `Result<_, E>`, so provide that type for `map_fn`
        pub(crate) fn map_eyre_only(
            self,
            map_fn: impl FnOnce(
                eyre::Result<std::convert::Infallible>,
            ) -> eyre::Result<std::convert::Infallible>,
        ) -> Self {
            let Self(kind) = self;
            match kind {
                TypedErrKind::Spawn(_) => Self(kind),
                TypedErrKind::Eyre(e) => {
                    let e = match map_fn(Err(e)) {
                        Ok(never) => match never {},
                        Err(e) => e,
                    };
                    Self(TypedErrKind::Eyre(e))
                }
            }
        }

        pub(crate) fn is_spawn_error(&self) -> bool {
            matches!(self, Self(TypedErrKind::Spawn(_)))
        }

        /// Extracts the [`eyre::Error`], meant to be called only in the final report location
        /// to avoid collapsing [`SpawnFail`] errors into generic [`eyre::Error`]s
        pub fn into_eyre_in_final_main_error_report_location(self) -> eyre::Error {
            let Self(kind) = self;
            match kind {
                TypedErrKind::Spawn(inner) => inner.into_eyre_in_final_main_error_report_location(),
                TypedErrKind::Eyre(inner) => inner,
            }
        }
    }
    impl From<SpawnFail> for TypedErr {
        fn from(value: SpawnFail) -> Self {
            Self(TypedErrKind::Spawn(value))
        }
    }
    impl From<eyre::Error> for TypedErr {
        fn from(value: eyre::Error) -> Self {
            Self(TypedErrKind::Eyre(value))
        }
    }
}

/// Runs the specified `cargo` command
///
/// # Errors
/// Returns an error if the command fails
pub fn run_cargo(args_fn: impl FnOnce(&mut Command) -> &mut Command) -> TypedResult<()> {
    let mut with_args = InterceptArgs::default();

    let status = status_cargo(|cmd| with_args.args_fn(args_fn, cmd))?;

    if !status.success() {
        crate::bail!("cargo command failed{with_args}")
    }
    Ok(())
}
/// Statuses the specified `cargo` command
///
/// # Errors
/// Returns an error if spawning the command fails
pub fn status_cargo(args_fn: impl FnOnce(&mut Command) -> &mut Command) -> TypedResult<ExitStatus> {
    let status = status_cmd(env!("CARGO"), args_fn)?;
    Ok(status)
}

mod run_cmd {
    use super::TypedResult;
    use std::process::Command;

    /// Runs the specified command
    ///
    /// # Errors
    /// Returns an error if the command fails
    pub fn run_cmd(
        cmd: &str,
        args_fn: impl FnOnce(&mut Command) -> &mut Command,
    ) -> TypedResult<()> {
        let status = super::status_cmd(cmd, args_fn)?;
        if !status.success() {
            // dbg!(&command);
            crate::bail!("`{cmd}` failed")
        }

        Ok(())
    }
}

struct DisplayCommand<'a>(&'a Command);
impl std::fmt::Display for DisplayCommand<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self(command) = self;

        let program = command.get_program();
        let args = std::fmt::from_fn(|f| f.debug_list().entries(command.get_args()).finish());

        #[expect(clippy::unnecessary_debug_formatting)]
        write!(f, "{program:?} with args {args:?}")?;
        if let Some(dir) = command.get_current_dir() {
            write!(f, " in {}", dir.display())?;
        }
        Ok(())
    }
}
fn dbg_command_run(command: &Command) {
    eprintln!("--> {}", DisplayCommand(command));
}

mod status_cmd {
    //! Low-level execution of commands

    use crate::{DisplayCommand, dbg_command_run};
    use std::process::{Command, ExitStatus};

    /// Runs the specified command and returns the [`ExitStatus`]
    ///
    /// # Errors
    /// Returns an error only if spawning the command fails
    pub fn status_cmd(
        cmd: &str,
        args_fn: impl FnOnce(&mut Command) -> &mut Command,
    ) -> Result<ExitStatus, SpawnFail> {
        let mut command = Command::new(cmd);
        args_fn(&mut command);
        dbg_command_run(&command);

        command.status().map_err(|source| {
            // dbg!(command);

            let err = SpawnError { source, command };
            // convert to `eyre::Error` to preserve the backtrace (if any)
            SpawnFail(eyre::eyre!(err))
        })
    }

    /// Wrapper around [`SpawnError`] that will not easily collapse into [`eyre::Error`], until the
    /// awkwardly named call to [`Self::into_eyre_in_final_main_error_report_location`]
    ///
    /// NOTE: The distinction (not collapsing this error into [`eyre::Error`] immediately) is
    /// intended help only show hints when the command spawns successfuly (e.g. but exits with a errors)
    #[derive(Debug)]
    pub struct SpawnFail(eyre::Error);
    impl SpawnFail {
        /// Extracts the [`eyre::Error`], meant to be called only in the final report location
        /// to avoid collapsing [`SpawnFail`] errors into generic [`eyre::Error`]s
        pub fn into_eyre_in_final_main_error_report_location(self) -> eyre::Error {
            let Self(inner) = self;
            inner
        }
    }

    /// Wrapper for [`std::io::Error`] from spawning a command
    #[derive(Debug)]
    pub struct SpawnError {
        source: std::io::Error,
        command: Command,
    }
    impl std::error::Error for SpawnError {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            let Self { source, .. } = self;
            Some(source)
        }
    }
    impl std::fmt::Display for SpawnError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let Self { source: _, command } = self;
            write!(f, "failed to run {}", DisplayCommand(command))
        }
    }
}

/// Prints the help note for applying fixes
pub fn print_help_fix_checks(source: &TypedErr, fix: &Option<Fix>) {
    if fix.is_some() {
        // don't print the hint, already in fix mode
        return;
    }
    if source.is_spawn_error() {
        // don't print the hint if spawning a tool failed (e.g. no `tsc` or `biome`)
        return;
    }

    let args = ["cargo xtask".to_owned()]
        .into_iter()
        .chain(std::env::args().skip(1))
        .chain(["fix".to_string()])
        .fold(String::new(), |mut acc, arg| {
            use std::fmt::Write as _;
            if !acc.is_empty() {
                write!(acc, " ").expect("infallible");
            }
            write!(acc, "{arg}").expect("infallible");
            acc
        });
    eprintln!("NOTE: Apply fixes using `{args}`");
}

/// Returns the root project folder path
///
/// # Panics
/// Panics at runtime if compiled with an invalid `CARGO_MANIFEST_DIR`
#[must_use]
pub fn project_root() -> PathBuf {
    Path::new(&env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(1)
        .expect("CARGO_MANIFEST_DIR should have at least one ancestor")
        .to_path_buf()
}

#[derive(Default)]
struct InterceptArgs {
    args_opt: Option<Vec<std::ffi::OsString>>,
}
impl InterceptArgs {
    fn args_fn<'a>(
        &mut self,
        args_fn: impl FnOnce(&mut Command) -> &mut Command,
        cmd: &'a mut Command,
    ) -> &'a mut Command {
        let Self { args_opt } = self;

        let cmd = args_fn(cmd);

        let args: Vec<_> = cmd.get_args().map(Into::into).collect();
        args_opt.replace(args);

        cmd
    }
}
impl std::fmt::Display for InterceptArgs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { args_opt } = self;
        let Some(args) = args_opt else {
            // for diagnostics only, so omit info if not available
            return Ok(());
        };
        write!(f, " with args:")?;
        for arg in args {
            #[allow(clippy::unnecessary_debug_formatting)]
            write!(f, " {arg:?}")?;
        }
        Ok(())
    }
}
