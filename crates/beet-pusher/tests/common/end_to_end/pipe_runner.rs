// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use std::{
    io::Read,
    process::{Child, Command, ExitStatus},
    str::FromStr,
};

use eyre::Context as _;
use fake_beet::create_config_file;

/// Spawns `bucket-spigot` in piped mode, collecting stdout and accepting inputs to forward to stdin
pub struct PipeRunner {
    cmd: Child,
    _temp_dir: tempfile::TempDir,
}
impl PipeRunner {
    pub fn spawn(
        vlc: &fake_vlc::FakeVlc,
        fake_beet_config: &fake_beet::ConfigAll,
    ) -> eyre::Result<Self> {
        let temp_dir = tempfile::tempdir().context("failed to create tempdir")?;
        let dir = temp_dir.path();

        let vlc_auth = vlc.get_auth_cloned();
        let vlc_auth_file = create_config_file(
            dir,
            "vlc_auth.toml",
            &toml::to_string_pretty(&vlc_auth).expect("failed vlc_auth toml serialize"),
        )?;

        create_config_file(
            dir,
            "beet-pusher.config.toml",
            &format!(
                r#"
                base_url="file://base_url"
                beet={beet:?}
                "#,
                beet = env!("CARGO_BIN_EXE_beet"),
            ),
        )?;

        let fake_beet_config_file =
            fake_beet_config.create_config_file(dir, "fake-beet-config.json")?;

        let cmd = Command::new(env!("CARGO_BIN_EXE_beet-pusher"))
            .arg("--json")
            .env("VLC_AUTH_FILE", vlc_auth_file)
            .env(fake_beet::FAKE_BEET_CONFIG_FILE, fake_beet_config_file)
            .current_dir(dir)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .context("failed to spawn beet-pusher")?;
        Ok(Self {
            cmd,
            _temp_dir: temp_dir,
        })
    }
    pub fn send_stdin(&mut self, lines: &JsonLines) -> eyre::Result<()> {
        let JsonLines(values) = lines;
        for value in values {
            self.send_stdin_line(value)?;
        }
        Ok(())
    }
    pub fn send_stdin_line<T>(&mut self, line: &T) -> eyre::Result<()>
    where
        T: std::fmt::Display + ?Sized,
    {
        use std::io::Write as _;

        let stdin = self.cmd.stdin.as_mut().expect("stdin available");
        writeln!(stdin, "{line}").with_context(|| {
            let line = line.to_string();
            format!("failed to serialize to stdin: {line:?}")
        })?;

        Ok(())
    }
    pub fn wait_success(self) -> eyre::Result<Output> {
        let (output, exit_status) = self.wait_for_result()?;

        if !exit_status.success() {
            let Output {
                stdout,
                stdout_json_lines: _,
                stderr,
            } = output;
            eprintln!("STDOUT:\n{stdout}\nEND");
            eprintln!("STDERR:\n{stderr}\nEND");
            eyre::bail!("subprocess exited with code {exit_status:?}");
        }

        Ok(output)
    }
    pub fn wait_for_result(self) -> eyre::Result<(Output, ExitStatus)> {
        let Self {
            mut cmd,
            _temp_dir: _,
        } = self;

        let stdin = cmd.stdin.take();
        drop(stdin);

        std::thread::sleep(std::time::Duration::from_millis(100));

        cmd.kill().context("failed to kill subprocess")?;
        let exit_status = cmd.wait().context("failed to wait for subprocess")?;

        let mut stdout_pipe = cmd.stdout.take().expect("stdout available");
        let mut stdout = String::new();
        stdout_pipe
            .read_to_string(&mut stdout)
            .context("failed to read stdout")?;

        let stdout_json_lines = stdout.parse();

        let mut stderr_pipe = cmd.stderr.take().expect("stderr available");
        let mut stderr = String::new();
        stderr_pipe
            .read_to_string(&mut stderr)
            .context("failed to read stderr")?;

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

pub struct Output {
    pub stdout: String,
    /// left as `Result` in case the test-caller tolerates non-JSON output
    pub stdout_json_lines: Result<JsonLines, serde_json::Error>,
    pub stderr: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use = "call `verify` to check against stdout"]
pub struct JsonLines(Vec<serde_json::Value>);
impl JsonLines {
    pub fn one(value: serde_json::Value) -> Self {
        Self(vec![value])
    }
    pub fn new(values: impl IntoIterator<Item = serde_json::Value>) -> Self {
        Self(values.into_iter().collect())
    }
    /// Converts the supplied stdout to [`JsonLines`], then asserts equal to self
    ///
    /// Useful with the result field in [`Output`]
    #[track_caller]
    pub fn assert_eq_stdout<E>(&self, stdout: Result<Self, E>) -> eyre::Result<()>
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        let stdout = stdout.context("stdout was not valid JSON")?;
        assert!(
            &stdout == self,
            "assertion `stdout == self` failed:\n--> stdout values:\n{stdout}--> expected values:\n{self}"
        );
        Ok(())
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
