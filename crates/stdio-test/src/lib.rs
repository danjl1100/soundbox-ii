// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Helpers for running a subcommand, sending to stdin, and receiving stdout

use std::{
    io::Read as _,
    path::Path,
    process::{Command, ExitStatus},
    str::FromStr,
};

use self::stdio_child::StdioChild;

use eyre::Context as _;

/// Spawns a command in piped mode, collecting stdout and accepting inputs to forward to stdin
pub struct StdioCmd {
    cmd: StdioChild,
    _temp_dir: tempfile::TempDir,
}
impl StdioCmd {
    /// Spawns the command
    ///
    /// # Errors
    /// Returns an error if the provided `setup_fn` or command spawn fails
    pub fn spawn(setup_fn: impl FnOnce(&Path) -> eyre::Result<Command>) -> eyre::Result<Self> {
        Self::spawn_with(|dir| {
            let cmd = setup_fn(dir)?;
            Ok((cmd, ()))
        })
        .map(|(this, ())| this)
    }
    /// Spawns the command
    ///
    /// # Errors
    /// Returns an error if the provided `setup_fn` or command spawn fails
    pub fn spawn_with<T>(
        setup_fn: impl FnOnce(&Path) -> eyre::Result<(Command, T)>,
    ) -> eyre::Result<(Self, T)> {
        let temp_dir = tempfile::tempdir().context("failed to create tempdir")?;
        let dir = temp_dir.path();

        let (mut cmd, extra) = setup_fn(dir)?;

        cmd.current_dir(dir);
        let cmd = StdioChild::spawn(cmd).context("failed to spawn command")?;

        let this = Self {
            cmd,
            _temp_dir: temp_dir,
        };
        Ok((this, extra))
    }
    /// Sends JSON lines to stdin
    ///
    /// # Errors
    /// Returns an error if the IO fails
    pub fn send_stdin(&mut self, lines: &JsonLines) -> eyre::Result<()> {
        let JsonLines(values) = lines;
        for value in values {
            self.send_stdin_line(value)?;
        }
        Ok(())
    }
    /// Sends one line to stdin
    ///
    /// # Errors
    /// Returns an error if the write fails
    #[track_caller]
    pub fn send_stdin_line<T>(&mut self, line: &T) -> eyre::Result<()>
    where
        T: std::fmt::Display + ?Sized,
    {
        use std::io::Write as _;

        let stdin = self.cmd.stdin_mut();
        writeln!(stdin, "{line}").with_context(|| {
            let line = line.to_string();
            format!("failed to serialize to stdin: {line:?}")
        })?;

        Ok(())
    }
    /// Convenience for [`Self::wait_for_result`] erroring on exit status is a
    /// failure
    ///
    /// # Errors
    /// Returns an error if reading the process output fails
    pub fn wait_success(
        self,
        timeout: std::time::Duration,
    ) -> eyre::Result<Result<Output, ExitStatusError>> {
        let (output, exit_status) = self.wait_for_result(timeout)?;

        if exit_status.success() {
            Ok(Ok(output))
        } else {
            let Output {
                stdout,
                stdout_json_lines: _,
                stderr,
            } = output;
            eprintln!("STDOUT:\n{stdout}\nEND");
            eprintln!("STDERR:\n{stderr}\nEND");
            Ok(Err(ExitStatusError { exit_status }))
        }
    }
    /// Closes stdin, drains stdout/stderr, then waits for the process to exit
    /// before killing the process
    ///
    /// # Errors
    /// Returns an error if reading the process result or output fails
    #[expect(clippy::missing_panics_doc, reason = "propagates output thread panics")]
    pub fn wait_for_result(
        self,
        timeout: std::time::Duration,
    ) -> eyre::Result<(Output, ExitStatus)> {
        let Self { cmd, _temp_dir: _ } = self;

        let (stdin, stdout, stderr, mut cmd) = cmd.into_parts();

        drop(stdin);

        // Drain pipes concurrently — read_to_string blocks until the write-end
        // closes (i.e. the child exits), so this captures all output reliably
        // regardless of platform buffering timing.
        let stdout_thread = std::thread::spawn(move || {
            let mut stdout = stdout;

            let mut buf = String::new();
            stdout.read_to_string(&mut buf).map(|_| buf)
        });
        let stderr_thread = std::thread::spawn(move || {
            let mut stderr = stderr;

            let mut buf = String::new();
            stderr.read_to_string(&mut buf).map(|_| buf)
        });

        // Wait for natural exit (stdin close should cause this), kill only as fallback
        let deadline = std::time::Instant::now() + timeout;
        loop {
            if cmd
                .try_wait()
                .context("failed to check subprocess status")?
                .is_some()
            {
                break;
            }
            if std::time::Instant::now() >= deadline {
                cmd.kill().context("failed to kill subprocess")?;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let exit_status = cmd.wait().context("failed to wait for subprocess")?;

        // Joining after wait() guarantees the pipe write-ends are closed, so
        // these joins return immediately with complete output.
        let stdout = stdout_thread
            .join()
            .expect("stdout thread panicked")
            .context("failed to read stdout")?;
        let stderr = stderr_thread
            .join()
            .expect("stderr thread panicked")
            .context("failed to read stderr")?;

        let stdout_json_lines = stdout.parse();
        Ok((
            Output {
                stdout,
                stdout_json_lines,
                stderr,
            },
            exit_status,
        ))
    }
}

/// Error from [`StdioCmd::wait_success`]
#[derive(Debug)]
pub struct ExitStatusError {
    exit_status: ExitStatus,
}
impl std::error::Error for ExitStatusError {}
impl std::fmt::Display for ExitStatusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { exit_status } = self;
        write!(f, "subprocess exited with code {exit_status:?}")
    }
}

/// Stdout and stderr from a process, with fallible JSON-parsed stdout lines
pub struct Output {
    /// Stdout as a (lossy) UTF-8 string
    pub stdout: String,
    /// Stdout as [`JsonLines`], left as `Result` in case the test-caller
    /// tolerates non-JSON output
    pub stdout_json_lines: Result<JsonLines, serde_json::Error>,
    /// Stderr as a (lossy) UTF-8 string
    pub stderr: String,
}

/// Lines of JSON, for sending to stdin or verifying stdout
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use = "call `verify` to check against stdout"]
pub struct JsonLines(Vec<serde_json::Value>);
impl JsonLines {
    /// Constructs from a single JSON line
    pub fn one(value: serde_json::Value) -> Self {
        Self(vec![value])
    }
    /// Constructs from multiple JSON lines
    pub fn new(values: impl IntoIterator<Item = serde_json::Value>) -> Self {
        Self(values.into_iter().collect())
    }
}
impl FromIterator<serde_json::Value> for JsonLines {
    fn from_iter<T: IntoIterator<Item = serde_json::Value>>(iter: T) -> Self {
        Self::new(iter)
    }
}
impl FromStr for JsonLines {
    type Err = serde_json::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.lines()
            .map(serde_json::from_str)
            .collect::<Result<_, _>>()
            .map(Self)
    }
}
impl std::fmt::Display for JsonLines {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self(values) = self;
        for value in values {
            writeln!(f, "{value}")?;
        }
        Ok(())
    }
}

impl JsonLines {
    /// Unwraps the supplied [`JsonLines`] result, then asserts equal to self
    ///
    /// Useful with the `stdout_json_lines` result field in [`Output`]
    ///
    /// # Errors
    /// Returns an error if the supplied stdout result is an error
    ///
    /// Returns an inner error if the content does not match self
    pub fn check_eq_stdout<E>(
        &self,
        stdout: Result<Self, E>,
    ) -> eyre::Result<Result<(), EqStdoutError>>
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        let stdout = stdout.context("stdout was not valid JSON")?;
        if &stdout != self {
            return Ok(Err(EqStdoutError {
                stdout,
                expected: self.clone(),
            }));
        }
        Ok(Ok(()))
    }
}
/// Error from [`JsonLines::check_eq_stdout`]
#[derive(Debug)]
pub struct EqStdoutError {
    stdout: JsonLines,
    expected: JsonLines,
}
impl std::error::Error for EqStdoutError {}
impl std::fmt::Display for EqStdoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { stdout, expected } = self;

        writeln!(f, "assertion `stdout == self` failed:")?;
        writeln!(f, "--> stdout values:\n{stdout}")?;
        writeln!(f, "--> expected values:\n{expected}")?;

        Ok(())
    }
}

mod stdio_child {
    use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command};

    /// [`std::process::Child`] with guaranteed access to stdin, stdout, and sterrr
    pub(super) struct StdioChild(Child);
    impl StdioChild {
        pub fn spawn(mut cmd: Command) -> std::io::Result<Self> {
            let child = cmd
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()?;

            assert!(child.stdin.is_some());
            assert!(child.stdout.is_some());
            assert!(child.stderr.is_some());

            Ok(Self(child))
        }
        pub fn stdin_mut(&mut self) -> &mut ChildStdin {
            let Self(inner) = self;
            inner
                .stdin
                .as_mut()
                .expect("stdin ensured by CmdStdio wrapper")
        }
        /// Returns the stdio parts, along with the stripped process handle
        pub fn into_parts(self) -> (ChildStdin, ChildStdout, ChildStderr, Child) {
            let Self(mut inner) = self;

            let stdin = inner.stdin.take();
            let stdout = inner.stdout.take();
            let stderr = inner.stderr.take();

            (
                stdin.expect("stdin ensured by CmdStdio wrapper"),
                stdout.expect("stdout ensured by CmdStdio wrapper"),
                stderr.expect("stderr ensured by CmdStdio wrapper"),
                inner,
            )
        }
    }
}
