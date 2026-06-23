// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Helpers for running a subcommand, sending to stdin, and receiving stdout

use std::{
    path::Path,
    process::{Command, ExitStatus},
    str::FromStr,
};

use crate::stdio_child::SpawnResult;

use eyre::Context as _;

/// String from stdout
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct OutStr(pub String);
/// String from stderr
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ErrStr(pub String);

impl std::fmt::Display for OutStr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self(inner) = self;
        write!(f, "{inner}")
    }
}
impl std::fmt::Display for ErrStr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self(inner) = self;
        write!(f, "{inner}")
    }
}
// Hack, but oh so convenient
impl std::ops::Deref for OutStr {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::Deref for ErrStr {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Signal to end the stdin forwarding loop
pub struct StdinShutdown;

type IoResultThread<T> = std::thread::JoinHandle<std::io::Result<T>>;

/// Spawns a command in piped mode, collecting stdout and accepting inputs to forward to stdin
pub struct StdioCmd {
    cmd: std::process::Child,
    stdin_tx: std::sync::mpsc::SyncSender<Result<String, StdinShutdown>>,
    stdin_thread: IoResultThread<()>,
    stdout_thread: IoResultThread<OutStr>,
    stderr_thread: IoResultThread<ErrStr>,
    _temp_dir: tempfile::TempDir,
}
/// Channels to send stdin lines and observe stdout and stderr lines
pub struct OutObserver {
    /// Sends lines to stdin
    pub stdin_tx: std::sync::mpsc::SyncSender<Result<String, StdinShutdown>>,
    /// Receives lines from stdout
    pub stdout_rx: std::sync::mpsc::Receiver<OutStr>,
    /// Receives lines from stderr
    pub stderr_rx: std::sync::mpsc::Receiver<ErrStr>,
}
impl StdioCmd {
    /// Spawns the command
    ///
    /// # Errors
    /// Returns an error if the provided `setup_fn` or command spawn fails
    pub fn spawn(
        setup_fn: impl FnOnce(&Path) -> eyre::Result<Command>,
    ) -> eyre::Result<(Self, OutObserver)> {
        Self::spawn_with(|dir| {
            let cmd = setup_fn(dir)?;
            Ok((cmd, ()))
        })
        .map(|(this, out, ())| (this, out))
    }
    /// Spawns the command
    ///
    /// # Errors
    /// Returns an error if the provided `setup_fn` or command spawn fails
    pub fn spawn_with<T>(
        setup_fn: impl FnOnce(&Path) -> eyre::Result<(Command, T)>,
    ) -> eyre::Result<(Self, OutObserver, T)> {
        let temp_dir = tempfile::tempdir().context("failed to create tempdir")?;
        let dir = temp_dir.path();

        let (mut cmd, extra) = setup_fn(dir)?;

        cmd.current_dir(dir);
        let SpawnResult {
            child: cmd,
            stdin: (stdin_tx, stdin_thread),
            stdout: (stdout_rx, stdout_thread),
            stderr: (stderr_rx, stderr_thread),
        } = stdio_child::spawn(cmd).context("failed to spawn command")?;

        let this = Self {
            cmd,
            stdin_tx: stdin_tx.clone(),
            stdin_thread,
            stdout_thread,
            stderr_thread,
            _temp_dir: temp_dir,
        };

        let out = OutObserver {
            stdin_tx,
            stdout_rx,
            stderr_rx,
        };

        Ok((this, out, extra))
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
        let line = line.to_string();
        self.stdin_tx
            .send(Ok(line))
            .context("failed to queue stdin line")?;

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
        let Self {
            mut cmd,
            stdin_tx,
            stdin_thread,
            stdout_thread,
            stderr_thread,
            _temp_dir: _,
        } = self;

        let _ = stdin_tx.send(Err(StdinShutdown));
        drop(stdin_tx);

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

        // join stdin thread after process ends
        stdin_thread.join().expect("stdin thread panicked")?;

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
    pub stdout: OutStr,
    /// Stdout as [`JsonLines`], left as `Result` in case the test-caller
    /// tolerates non-JSON output
    pub stdout_json_lines: Result<JsonLines, serde_json::Error>,
    /// Stderr as a (lossy) UTF-8 string
    pub stderr: ErrStr,
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
    use std::{
        io::BufReader,
        process::{Child, Command},
    };

    use crate::{ErrStr, OutStr, StdinShutdown};

    type ReceiverAndThread<T> = (
        std::sync::mpsc::Receiver<T>,
        std::thread::JoinHandle<std::io::Result<T>>,
    );

    /// [`std::process::Child`] with guaranteed access to stdin, and reads
    /// stdout and sterrr
    pub(super) struct SpawnResult {
        pub child: Child,
        pub stdin: (
            std::sync::mpsc::SyncSender<Result<String, StdinShutdown>>,
            std::thread::JoinHandle<std::io::Result<()>>,
        ),
        pub stdout: ReceiverAndThread<OutStr>,
        pub stderr: ReceiverAndThread<ErrStr>,
    }
    pub fn spawn(mut cmd: Command) -> std::io::Result<SpawnResult> {
        let mut child = cmd
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        let stdin = child.stdin.take().expect("stdin available");
        let stdout = child.stdout.take().expect("stdout available");
        let stderr = child.stderr.take().expect("stderr available");

        let (stdin_line_tx, stdin_line_rx) = std::sync::mpsc::sync_channel(1);
        let stdin_thread = std::thread::spawn(move || {
            use std::io::Write as _;

            let mut stdin = stdin;
            for line in stdin_line_rx {
                let line = match line {
                    Ok(line) => line,
                    Err(StdinShutdown) => break,
                };
                writeln!(&mut stdin, "{line}")?;
            }
            Ok(())
        });

        let (stdout_line_tx, stdout_line_rx) = std::sync::mpsc::channel();
        let stdout_thread =
            std::thread::spawn(move || read_stream(stdout, &stdout_line_tx, OutStr));
        let (stderr_line_tx, stderr_line_rx) = std::sync::mpsc::channel();
        let stderr_thread =
            std::thread::spawn(move || read_stream(stderr, &stderr_line_tx, ErrStr));

        Ok(SpawnResult {
            child,
            stdin: (stdin_line_tx, stdin_thread),
            stdout: (stdout_line_rx, stdout_thread),
            stderr: (stderr_line_rx, stderr_thread),
        })
    }

    fn read_stream<R, T>(
        stream: R,
        line_tx: &std::sync::mpsc::Sender<T>,
        label_fn: impl Fn(String) -> T,
    ) -> std::io::Result<T>
    where
        R: std::io::Read,
    {
        use std::fmt::Write as _;
        use std::io::BufRead as _;

        let reader = BufReader::new(stream);

        let mut buf = String::new();

        for line in reader.lines() {
            let line = line?;
            writeln!(&mut buf, "{line}").expect("string fmt infallible");
            let _ = line_tx.send(label_fn(line));
        }

        Ok(label_fn(buf))
    }
}
