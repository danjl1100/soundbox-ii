// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use std::process::Command;

use eyre::Context as _;
use stdio_test::{ExitStatusError, JsonLines, OutStr, Output, StdinShutdown, StdioCmd};

pub struct PipeRunner {
    cmd: StdioCmd,
    port: u16,
    respond_tx: std::sync::mpsc::Sender<ReplyPlan>,
    respond_thread: MpscSendThread<Result<String, StdinShutdown>>,
}
impl PipeRunner {
    pub fn spawn() -> eyre::Result<Self> {
        let (cmd, out_observer, port_file) = StdioCmd::spawn_with(|dir| {
            let port_file = dir.join("port_file.txt");

            let mut cmd = Command::new(env!("CARGO_BIN_EXE_beet-pusher-webui"));
            cmd.env("SCRIPT_WRITE_PORT", &port_file)
                .env("PORT", "0")
                .env("RUST_LOG", "beet_pusher=DEBUG");
            Ok((cmd, port_file))
        })?;

        // read port from `port_file`
        let start = std::time::Instant::now();
        let port = loop {
            let read_err = match Self::read_port(&port_file) {
                Ok(port) => break port,
                Err(e) => e,
            };
            if start.elapsed() > std::time::Duration::from_secs(1) {
                Err(read_err).context("timed out waiting for port_file")?;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        };

        // spawn thread to respond to known stdout messages
        let (respond_tx, respond_thread) =
            Self::spawn_respond(out_observer.stdin_tx, out_observer.stdout_rx);

        let this = Self {
            cmd,
            port,
            respond_tx,
            respond_thread,
        };

        Ok(this)
    }
    /// Queues a stdin pipe response if the request pattern is seen on stdout
    pub fn queue_reply(
        &self,
        reply: ReplyPlan,
    ) -> Result<(), std::sync::mpsc::SendError<ReplyPlan>> {
        self.respond_tx.send(reply)
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

        let Self {
            cmd,
            port: _,
            respond_tx,
            respond_thread,
        } = self;

        drop(respond_tx);

        let result = cmd.wait_success(WAIT_TIMEOUT);

        respond_thread.join().expect("panic in respond_thread")?;

        result
    }
}

impl PipeRunner {
    fn read_port(port_file: &std::path::Path) -> eyre::Result<u16> {
        let port = std::fs::read_to_string(port_file).context("failed to load port_file")?;
        let port = port.parse().context("invalid text in port_file")?;
        Ok(port)
    }
    pub fn http_post<T>(&mut self, path: &str, body: &T) -> eyre::Result<String>
    where
        T: serde::Serialize,
    {
        let body = serde_json::to_vec(body).context("failed to serialize body")?;

        let uri = format!("http://127.0.0.1:{port}/{path}", port = self.port);

        let config = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .build();
        let agent = ureq::Agent::from(config);

        let response = agent
            .post(&uri)
            .header("Content-Type", "application/json")
            .send(body)
            .context("failed to send request");

        let mut response = match response {
            Ok(response) => response,
            Err(e) => {
                dbg!(&e);
                eyre::bail!(e)
            }
        };

        let response = response.body_mut().read_to_string()?;
        tracing::debug!(uri, response);

        Ok(response)
    }
}

pub struct ReplyPlan {
    pub request_pattern: OutStr,
    pub response: String,
}
type MpscSendThread<T> = std::thread::JoinHandle<Result<(), std::sync::mpsc::SendError<T>>>;
impl PipeRunner {
    fn spawn_respond<E>(
        stdin_tx: std::sync::mpsc::SyncSender<Result<String, E>>,
        stdout_rx: std::sync::mpsc::Receiver<OutStr>,
    ) -> (
        std::sync::mpsc::Sender<ReplyPlan>,
        MpscSendThread<Result<String, E>>,
    )
    where
        E: Send + 'static,
    {
        let (respond_tx, respond_rx) = std::sync::mpsc::channel::<ReplyPlan>();

        let respond_thread = std::thread::spawn(move || {
            let mut responses = std::collections::BTreeMap::new();
            while let Ok(line) = stdout_rx.recv() {
                for reply_plan in respond_rx.try_iter() {
                    let entry = responses
                        .entry(reply_plan.request_pattern)
                        .or_insert_with(Vec::new);
                    entry.push(reply_plan.response);
                }

                if let Some(list) = responses.get_mut(&line)
                    && let Some(response) = list.pop()
                {
                    stdin_tx.send(Ok(response))?;
                }
                if let Some(list) = responses.get(&line)
                    && list.is_empty()
                {
                    responses.remove(&line);
                }
            }
            Ok(())
        });
        (respond_tx, respond_thread)
    }
}
