// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Logic for the `xtask` functionality

pub use self::quiet::{ArgsCmdSettings, Quiet};
pub use self::status_cmd::CmdSettings;
pub use self::status_cmd::{SpawnError, SpawnFail};
pub use self::typed_err::{TypedErr, TypedResult};
pub use self::write_output::{ArgFix, WriteOutput};
use eyre::Context as _;
use std::{
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
};

pub mod copyright;
pub mod rust;
pub mod spigot_visual;
pub mod supply_chain;
pub mod vlc;

#[cfg(unix)]
pub mod unix_exec;

mod write_output {
    /// The user wants to write files to improve the result
    #[derive(Clone, Copy, Debug)]
    pub struct WriteOutput {
        _sealed: (),
    }

    impl WriteOutput {
        /// Caller asserts that the user wants to write files
        ///
        /// NOTE: Long name to make it clear what the caller is asserting
        #[must_use]
        pub fn unchecked_user_wants_to_write_files() -> Self {
            Self { _sealed: () }
        }
    }

    #[derive(Clone, Debug, clap::Args)]
    pub(crate) struct ArgFixJsFmt {
        /// Write formatting fixes to JavaScript source files
        #[clap(long)]
        fix: bool,
    }
    impl ArgFixJsFmt {
        /// Returns the inner typed value
        pub fn into_inner(self) -> Option<WriteOutput> {
            let Self { fix } = self;
            fix.then_some(WriteOutput { _sealed: () })
        }
    }

    /// clap builder for [`WriteOutput`] in the context of `--fix` in checks
    #[derive(Clone, Debug, clap::Args)]
    pub struct ArgFix {
        /// Attempt to fix checks by writing to files (otherwise, run read-only checks)
        #[clap(long)]
        fix: bool,
    }
    impl ArgFix {
        /// Returns the inner typed value
        #[must_use]
        pub fn into_inner(self) -> Option<WriteOutput> {
            let Self { fix } = self;
            fix.then_some(WriteOutput { _sealed: () })
        }
    }
}

mod quiet {
    use crate::CmdSettings;

    /// The user wants to suppress output from commands if no errors occur
    #[derive(Clone, Copy, Debug)]
    pub struct Quiet {
        _sealed: (),
    }

    /// clap entrypoint builder for [`Quiet`]
    #[derive(Clone, Debug, clap::Args)]
    pub struct ArgsCmdSettings {
        /// Suppress subcommand output unless an error occurs
        #[clap(long, env = "XTASK_QUIET")]
        quiet: bool,
    }
    impl ArgsCmdSettings {
        /// Returns the inner typed value
        #[must_use]
        pub fn into_inner(self) -> CmdSettings {
            let Self { quiet } = self;
            let quiet = quiet.then_some(Quiet { _sealed: () });
            CmdSettings::new(quiet)
        }
    }
}

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

impl CmdSettings {
    /// Runs the specified `cargo` command
    ///
    /// # Errors
    /// Returns an error if the command fails
    pub fn run_cargo(&self, args_fn: impl FnOnce(&mut Command) -> &mut Command) -> TypedResult<()> {
        let mut with_args = InterceptArgs::default();

        let status = self.status_cargo(|cmd| with_args.args_fn(args_fn, cmd))?;

        if !status.success() {
            crate::bail!("cargo command failed{with_args}")
        }
        Ok(())
    }
    /// Statuses the specified `cargo` command
    ///
    /// # Errors
    /// Returns an error if spawning the command fails
    pub fn status_cargo(
        &self,
        args_fn: impl FnOnce(&mut Command) -> &mut Command,
    ) -> TypedResult<ExitStatus> {
        let quiet = self.get_quiet();
        let status = self.status_cmd(env!("CARGO"), |c| {
            if let Some(Quiet { .. }) = quiet {
                c.arg("--quiet");
            }
            args_fn(c)
        })?;
        Ok(status)
    }
    /// Runs the specified command
    ///
    /// # Errors
    /// Returns an error if the command fails
    pub fn run_cmd(
        &self,
        cmd: &str,
        args_fn: impl FnOnce(&mut Command) -> &mut Command,
    ) -> TypedResult<()> {
        let status = self.status_cmd(cmd, args_fn)?;
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

        write!(f, "{} with args {args:?}", program.display())?;
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

    use crate::{DisplayCommand, Quiet, dbg_command_run};
    use std::process::{Command, ExitStatus};

    /// Settings for running [`Command`]s
    #[derive(Debug)]
    pub struct CmdSettings {
        quiet: Option<Quiet>,
    }
    impl CmdSettings {
        /// Creates settings for running subcommands
        #[must_use]
        pub fn new(quiet: Option<Quiet>) -> Self {
            Self { quiet }
        }
        /// Returns the [`Quiet`] setting
        #[must_use]
        pub fn get_quiet(&self) -> Option<Quiet> {
            self.quiet
        }
        /// Runs the specified command and returns the [`ExitStatus`]
        ///
        /// # Errors
        /// Returns an error only if spawning the command fails
        pub fn status_cmd(
            &self,
            cmd: &str,
            args_fn: impl FnOnce(&mut Command) -> &mut Command,
        ) -> Result<ExitStatus, SpawnFail> {
            let Self { quiet } = self;

            let mut command = Command::new(cmd);
            args_fn(&mut command);
            dbg_command_run(&command);

            if let Some(Quiet { .. }) = quiet {
                // TODO: suppress output, and only print to stdout/stderr if the command fails
                unimplemented!("quiet mode");
            }

            command.status().map_err(|source| {
                // dbg!(command);

                let err = SpawnError { source, command };
                // convert to `eyre::Error` to preserve the backtrace (if any)
                SpawnFail(eyre::eyre!(err))
            })
        }
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
pub fn print_help_fix_checks(source: &TypedErr, fix: &Option<WriteOutput>) {
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
        .chain(["--fix".to_string()])
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
            write!(f, " {}", arg.display())?;
        }
        Ok(())
    }
}

/// Repeats the prompt until the user responds with yes (with a hint to Ctrl+C to exit)
///
/// # Errors
/// Returns if the I/O fails to the user prompt
fn confirm_until_yes(prompt: &impl std::fmt::Display) -> eyre::Result<()> {
    loop {
        match confirm(prompt).context("I/O for prompt failed")? {
            Ok(()) => return Ok(()),
            Err(response) => {
                println!("Expected \"y\" or \"yes\", not: {response:?} (Ctrl+C to exit)");
            }
        }
    }
}
/// Prompts the specified question, returning `Ok(Ok(()))` on "y" or "yes" (ascii case insensitive),
/// or `Ok(Err(response))` on all other responses.
///
/// # Errors
/// Returns an error if writing stdout or reading stdin fails
fn confirm(prompt: &impl std::fmt::Display) -> std::io::Result<Result<(), String>> {
    use std::io::Write as _;

    {
        let mut stdout = std::io::stdout().lock();
        write!(&mut stdout, "{prompt} [y/N]: ")?;
        stdout.flush()?;
    }

    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;

    let line = line.trim();

    match &*line.to_ascii_lowercase() {
        "y" | "yes" => Ok(Ok(())),
        _ => Ok(Err(line.to_string())),
    }
}
