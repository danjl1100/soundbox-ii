// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use std::process::{Command, ExitStatus};

use fake_beet::create_config_file;
use stdio_test::{ExitStatusError, StdioCmd};
pub use stdio_test::{JsonLines, Output};

/// Spawns `beet-pusher` in piped mode, collecting stdout and accepting inputs to forward to stdin
pub struct PipeRunner {
    cmd: StdioCmd,
}
pub struct Builder<'a> {
    vlc_auth: vlc_http_auth::AuthInput,
    fake_beet_config: &'a fake_beet::ConfigAll,
    vlc_http_timeout_millis: Option<u64>,
}
impl PipeRunner {
    pub fn build<'a>(
        vlc: &fake_vlc::FakeVlc,
        fake_beet_config: &'a fake_beet::ConfigAll,
    ) -> Builder<'a> {
        let vlc_auth = vlc.get_auth_cloned();
        Builder {
            vlc_auth,
            fake_beet_config,
            vlc_http_timeout_millis: None,
        }
    }
}
impl Builder<'_> {
    /// Sets the additional HTTP timeout in milliseconds
    ///
    /// # Panics
    ///
    /// Panics if the input is less than 10, to avoid flakey tests
    #[track_caller]
    pub fn set_vlc_http_timeout_millis(&mut self, millis: u64) -> &mut Self {
        assert!(millis >= 10, "VLC HTTP client timeout must be non-zero");
        self.vlc_http_timeout_millis = Some(millis);
        self
    }
    /// Creates a string for the beet-pusher config file content
    fn render_config_content(&self) -> String {
        #[derive(serde::Serialize)]
        struct ConfigRender<'a> {
            base_url: &'a str,
            beet: std::borrow::Cow<'a, str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            vlc_http_timeout_millis: Option<u64>,
        }

        let Self {
            vlc_http_timeout_millis,
            ..
        } = *self;

        let value = ConfigRender {
            base_url: "file://base_url",
            beet: fake_beet::build_bin_once().path().to_string_lossy(),
            vlc_http_timeout_millis,
        };
        toml::to_string_pretty(&value).expect("ConfigRender to TOML should be infallible")
    }
}
impl Builder<'_> {
    pub fn spawn(&self) -> eyre::Result<PipeRunner> {
        let Self {
            vlc_auth,
            fake_beet_config,
            vlc_http_timeout_millis: _, // read in [`render_config_content`]
        } = self;

        StdioCmd::spawn(|dir| {
            let vlc_auth_file = create_config_file(
                dir,
                "vlc_auth.toml",
                &toml::to_string_pretty(&vlc_auth).expect("failed vlc_auth toml serialize"),
            )?;

            create_config_file(
                dir,
                "beet-pusher.config.toml",
                &self.render_config_content(),
            )?;

            let fake_beet_config_file =
                fake_beet_config.create_config_file(dir, "fake-beet-config.json")?;

            let mut cmd = Command::new(env!("CARGO_BIN_EXE_beet-pusher"));
            cmd.arg("--json")
                .env("VLC_AUTH_FILE", vlc_auth_file)
                .env(fake_beet::FAKE_BEET_CONFIG_FILE, fake_beet_config_file);

            Ok(cmd)
        })
        .map(|(cmd, _out_observer)| PipeRunner { cmd })
    }
}
impl PipeRunner {
    pub fn send_stdin(&mut self, lines: &JsonLines) -> eyre::Result<()> {
        self.cmd.send_stdin(lines)
    }
    pub fn send_stdin_line<T>(&mut self, line: &T) -> eyre::Result<()>
    where
        T: std::fmt::Display + ?Sized,
    {
        self.cmd.send_stdin_line(line)
    }
}

const WAIT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
impl PipeRunner {
    pub fn wait_success(self) -> eyre::Result<Result<Output, ExitStatusError>> {
        self.cmd.wait_success(WAIT_TIMEOUT)
    }
    pub fn wait_for_result(self) -> eyre::Result<(Output, ExitStatus)> {
        self.cmd.wait_for_result(WAIT_TIMEOUT)
    }
}
