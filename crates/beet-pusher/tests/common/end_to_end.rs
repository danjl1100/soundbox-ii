// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use crate::common::end_to_end::pipe_runner::{Output, PipeRunner};

#[test]
#[ignore = "TODO"]
fn stdin_reports_unknown_command() -> eyre::Result<()> {
    let (vlc, vlc_thread) = fake_vlc::FakeVlc::new()?;

    let vlc_auth = vlc.get_auth_cloned();
    let mut r = PipeRunner::spawn(&vlc_auth)?;

    r.send_stdin("test string is **NOT** a JSON object")?;

    let Output { stdout } = r.wait()?;

    assert_eq!(
        stdout,
        r#"{"error":"invalid command: \"test string is **NOT** a JSON object\""}"#
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
        process::{Child, Command},
    };

    use eyre::Context as _;
    use fake_beet::create_config_file;

    /// Spawns `bucket-spigot` in piped mode, collecting stdout and accepting inputs to forward to stdin
    pub struct PipeRunner {
        cmd: Child,
        _temp_dir: tempfile::TempDir,
    }
    impl PipeRunner {
        pub fn spawn(vlc_auth: &vlc_http_auth::AuthInput) -> eyre::Result<Self> {
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

            let fake_beet_config_file = {
                let mut fake_beet_config = fake_beet::ConfigAll::default();
                fake_beet_config
                    .for_args(["ls", "-f$id=$path", "grouping:1|2|3|4|5", "has_lyrics::^$"])
                    .stdout_lines(["1=/path/to/file1.mp3", "2=/path/to/file2.mp3"]);
                fake_beet_config
                    .for_args(["ls", "-f$id=$path", "added:2020..", "grouping::^$"])
                    .stdout_lines(["5=/path/recent_file1.mp3", "6=/path/recent_file2.mp3"]);
                fake_beet_config
                    .for_args(["ls", "-f$id=$path", "grouping::1|2|3|4|5", "has_lyrics::^$"])
                    .stdout_lines(["7=/path/lyrics_file1.mp3"]);
                fake_beet_config.create_config_file(dir, "fake-beet-config.json")?
            };

            let cmd = Command::new(env!("CARGO_BIN_EXE_beet-pusher"))
                .arg("--json")
                .env("VLC_AUTH_FILE", vlc_auth_file)
                .env(fake_beet::FAKE_BEET_CONFIG_FILE, fake_beet_config_file)
                .current_dir(dir)
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .spawn()
                .context("failed to spawn beet-pusher")?;
            Ok(Self {
                cmd,
                _temp_dir: temp_dir,
            })
        }
        pub fn send_stdin<T>(&mut self, line: &T) -> eyre::Result<()>
        where
            T: serde::Serialize + std::fmt::Debug + ?Sized,
        {
            let stdin = self.cmd.stdin.as_mut().expect("stdin available");
            serde_json::to_writer(stdin, line)
                .with_context(|| format!("failed to serialize to stdin: {line:?}"))?;

            Ok(())
        }
        pub fn wait(self) -> eyre::Result<Output> {
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

            if !exit_status.success() {
                eprintln!("STDOUT:\n{stdout}\nEND");
                eyre::bail!("subprocess exited with code {exit_status:?}");
            }

            Ok(Output { stdout })
        }
    }

    pub struct Output {
        pub stdout: String,
    }
}
