// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use crate::common::end_to_end::pipe_runner::{Output, PipeRunner};

#[test]
fn stdin_reports_unknown_command() -> eyre::Result<()> {
    let (vlc, vlc_thread) = fake_vlc::FakeVlc::new()?;

    let fake_beet_config = fake_beet::ConfigAll::setup_with(|c| {
        c.for_args(["ls", "-f$id=$path", "grouping:1|2|3|4|5", "has_lyrics::^$"])
            .stdout_lines(["1=/path/to/file1.mp3", "2=/path/to/file2.mp3"]);
        c.for_args(["ls", "-f$id=$path", "added:2020..", "grouping::^$"])
            .stdout_lines(["5=/path/recent_file1.mp3", "6=/path/recent_file2.mp3"]);
        c.for_args(["ls", "-f$id=$path", "grouping::1|2|3|4|5", "has_lyrics::^$"])
            .stdout_lines(["7=/path/lyrics_file1.mp3"]);
    });

    let vlc_auth = vlc.get_auth_cloned();
    let mut r = PipeRunner::spawn(&vlc_auth, &fake_beet_config)?;

    r.send_stdin("test string is **NOT** a JSON object")?;

    let Output { stdout, stderr } = r.wait_success()?;

    assert_eq!(
        stdout,
        "{\"error\":{\"kind\":\"invalid_command\",\"command_json\":\"test string is **NOT** a JSON object\"}}\n"
    );

    assert!(
        stderr.contains("test string is **NOT** a JSON object"),
        "expected library error in stderr"
    );
    assert!(
        stderr.contains("expected value at line 1 column 1"),
        "expected JSON error in stderr"
    );

    drop(vlc);
    vlc_thread.join().expect("VLC thread panic")?;

    Ok(())
}

#[test]
#[ignore = "TODO"]
fn stdin_modify_spigot() -> eyre::Result<()> {
    eyre::bail!("TODO: send spigot modifications through stdin, verify status in stdout")
}

mod pipe_runner {
    use std::{
        io::Read,
        process::{Child, Command, ExitStatus},
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
            vlc_auth: &vlc_http_auth::AuthInput,
            fake_beet_config: &fake_beet::ConfigAll,
        ) -> eyre::Result<Self> {
            let temp_dir = tempfile::tempdir().context("failed to create tempdir")?;
            let dir = temp_dir.path();

            let vlc_auth_file = create_config_file(
                dir,
                "vlc_auth.toml",
                &toml::to_string_pretty(vlc_auth).expect("failed vlc_auth toml serialize"),
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
        pub fn send_stdin(&mut self, line: &str) -> eyre::Result<()> {
            use std::io::Write as _;

            let stdin = self.cmd.stdin.as_mut().expect("stdin available");
            writeln!(stdin, "{line}")
                // where T: serde::Serialize + std::fmt::Debug + ?Sized,
                // serde_json::to_writer(stdin, line)
                .with_context(|| format!("failed to serialize to stdin: {line:?}"))?;

            Ok(())
        }
        pub fn wait_success(self) -> eyre::Result<Output> {
            let (output, exit_status) = self.wait_for_result()?;

            if !exit_status.success() {
                let Output { stdout, stderr } = output;
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

            let mut stderr_pipe = cmd.stderr.take().expect("stderr available");
            let mut stderr = String::new();
            stderr_pipe
                .read_to_string(&mut stderr)
                .context("failed to read stderr")?;

            Ok((Output { stdout, stderr }, exit_status))
        }
    }

    pub struct Output {
        pub stdout: String,
        pub stderr: String,
    }
}
