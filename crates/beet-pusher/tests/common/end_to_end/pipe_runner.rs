// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use std::process::Command;

use fake_beet::create_config_file;
use stdio_test::StdioCmd;
pub use stdio_test::{JsonLines, Output};

/// Spawns `bucket-spigot` in piped mode, collecting stdout and accepting inputs to forward to stdin
pub struct PipeRunner {
    cmd: StdioCmd,
}
impl PipeRunner {
    pub fn spawn(
        vlc: &fake_vlc::FakeVlc,
        fake_beet_config: &fake_beet::ConfigAll,
    ) -> eyre::Result<Self> {
        StdioCmd::spawn(|dir| {
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

            let mut cmd = Command::new(env!("CARGO_BIN_EXE_beet-pusher"));
            cmd.arg("--json")
                .env("VLC_AUTH_FILE", vlc_auth_file)
                .env(fake_beet::FAKE_BEET_CONFIG_FILE, fake_beet_config_file);

            Ok(cmd)
        })
        .map(|cmd| Self { cmd })
    }
    pub fn send_stdin(&mut self, lines: &JsonLines) -> eyre::Result<()> {
        self.cmd.send_stdin(lines)
    }
    pub fn send_stdin_line<T>(&mut self, line: &T) -> eyre::Result<()>
    where
        T: std::fmt::Display + ?Sized,
    {
        self.cmd.send_stdin_line(line)
    }
    pub fn wait_success(self) -> eyre::Result<Output> {
        const WAIT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

        self.cmd.wait_success(WAIT_TIMEOUT)
    }
}
