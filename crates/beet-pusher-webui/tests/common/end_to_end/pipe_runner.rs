// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use std::{path::PathBuf, process::Command};

use stdio_test::{ExitStatusError, JsonLines, Output, StdioCmd};

pub struct PipeRunner {
    cmd: StdioCmd,
    /// HTTP listen port, or the file that will contain the port
    port_or_file: Result<u16, PathBuf>,
}
impl PipeRunner {
    pub fn spawn() -> eyre::Result<Self> {
        StdioCmd::spawn_with(|dir| {
            let port_file = dir.join("port_file.txt");

            let mut cmd = Command::new(env!("CARGO_BIN_EXE_beet-pusher-webui"));
            cmd.env("SCRIPT_WRITE_PORT", &port_file)
                .env("PORT", "0")
                .env("RUST_LOG", "beet_pusher=DEBUG");
            Ok((cmd, port_file))
        })
        .map(|(cmd, port_file)| Self {
            cmd,
            port_or_file: Err(port_file),
        })
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
    pub fn wait_success(self) -> eyre::Result<Result<Output, ExitStatusError>> {
        const WAIT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

        self.cmd.wait_success(WAIT_TIMEOUT)
    }
}

impl PipeRunner {
    // pub fn http(method: ureq)
}
