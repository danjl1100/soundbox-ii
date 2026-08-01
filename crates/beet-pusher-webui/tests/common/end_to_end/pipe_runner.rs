// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use std::process::Command;

use eyre::Context as _;
use stdio_test::{ExitStatusError, OutStr, Output, StdinMsg, StdioCmd};
use ureq::http;

const IP_ADDR_LOCAL: &str = "127.0.0.1";

pub struct PipeRunner {
    cmd: StdioCmd,
    port: u16,
    respond_tx: std::sync::mpsc::Sender<ReplyPlan>,
    respond_thread: MpscSendThread<StdinMsg, Vec<ReplyPlan>>,
}
impl PipeRunner {
    pub fn spawn() -> eyre::Result<Self> {
        let (cmd, out_observer, port_file) = StdioCmd::spawn_with(|dir| {
            let port_file = dir.join("port_file.txt");

            let mut cmd = Command::new(env!("CARGO_BIN_EXE_beet-pusher-webui"));
            cmd.env("BEET_PUSHER_WEBUI__SCRIPT_WRITE_PORT", &port_file)
                .env("BEET_PUSHER_WEBUI__PORT", "0")
                .env("BEET_PUSHER_WEBUI__BIND_IP", IP_ADDR_LOCAL)
                .env("RUST_LOG", "beet_pusher=DEBUG");
            Ok((cmd, port_file))
        })?;

        // POST-SETUP FUNCTION
        let post_setup_result = (move || {
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
            Ok(port)
        })();

        let port = match post_setup_result {
            Ok(port) => port,
            Err(e) => {
                let wait_result = cmd.wait_success(std::time::Duration::from_secs(1));
                match wait_result {
                    Ok(Ok(_output)) => {}
                    Ok(Err(e)) => eprintln!("{e:?}"),
                    Err(e) => eprintln!("{e:?}"),
                }
                return Err(e);
            }
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
}

type WaitSuccessResult = eyre::Result<Result<Output, ExitStatusError>>;

pub struct UutAndTestResult<T> {
    uut: eyre::Result<Output>,
    test: eyre::Result<T>,
    post_checks: Result<(), RemainingRepliesError>,
}
impl UutAndTestResult<()> {
    pub fn report_output(self) -> Result<Output, UutAndTestError> {
        let (output, ()) = self.report_all()?;
        Ok(output)
    }
}
impl<T> UutAndTestResult<T> {
    pub fn report_all(self) -> Result<(Output, T), UutAndTestError> {
        use UutAndTestError::{Many, One};

        let Self {
            uut,
            test,
            post_checks,
        } = self;
        match (uut, test) {
            (Ok(uut), Ok(test)) => post_checks
                .map(|()| (uut, test))
                .map_err(|e| UutAndTestError::One(e.into())),
            (Ok(_), Err(test)) => Err(One(test)),
            (Err(uut), Ok(_)) => Err(One(uut)),
            (Err(uut), Err(test)) => Err(Many { uut, test }),
        }
    }
}
#[derive(Debug)]
pub enum UutAndTestError {
    One(eyre::Error),
    Many { uut: eyre::Error, test: eyre::Error },
}
impl std::error::Error for UutAndTestError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::One(err) => err.source(),
            Self::Many { uut: _, test: _ } => None,
        }
    }
}
impl std::fmt::Display for UutAndTestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::One(inner) => write!(f, "{inner}"),
            Self::Many { uut, test } => {
                writeln!(f, "UUT and TEST failure")?;

                writeln!(f)?;
                writeln!(f, "TEST failure:")?;
                writeln!(f, "-------------")?;
                writeln!(f, "{test:?}")?;

                writeln!(f)?;
                writeln!(f, "UUT failure:")?;
                writeln!(f, "------------")?;
                writeln!(f, "{uut:?}")
            }
        }
    }
}

impl PipeRunner {
    pub fn run_then_wait_success<T>(
        mut self,
        test_fn: impl FnOnce(&mut Self) -> eyre::Result<T>,
    ) -> eyre::Result<UutAndTestResult<T>> {
        // first, run test
        let test_result = test_fn(&mut self);

        // then, wait for uut
        let (wait_result, remaining_replies_result) = self.wait_success_inner();

        let remaining_replies = remaining_replies_result?;
        let uut_result = wait_result?;

        let post_checks = Self::verify_empty_remaining_replies(remaining_replies);

        Ok(UutAndTestResult {
            uut: uut_result.map_err(Into::into),
            test: test_result,
            post_checks,
        })
    }
    pub fn wait_success(self) -> eyre::Result<Result<Output, WaitSuccessError>> {
        let (wait_result, remaining_replies_result) = self.wait_success_inner();

        // LEVEL 1 - test harness failures
        let remaining_replies = remaining_replies_result?;
        let output_result = wait_result?;

        let uut_result = (|| {
            // LEVEL 2 - UUT execution failure
            let output = output_result?;

            // LEVEL 3 - output postconditions
            Self::verify_empty_remaining_replies(remaining_replies)?;

            Ok(output)
        })();

        Ok(uut_result)
    }
    fn verify_empty_remaining_replies(
        remaining_replies: Vec<ReplyPlan>,
    ) -> Result<(), RemainingRepliesError> {
        if !remaining_replies.is_empty() {
            return Err(RemainingRepliesError { remaining_replies });
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WaitSuccessError {
    #[error(transparent)]
    ExitStatus(#[from] stdio_test::ExitStatusError),
    #[error(transparent)]
    RemainingReplies(#[from] RemainingRepliesError),
}

#[derive(Debug, thiserror::Error)]
#[error("unused replies in the queue: {remaining_replies:?}")]
pub struct RemainingRepliesError {
    remaining_replies: Vec<ReplyPlan>,
}

impl PipeRunner {
    fn read_port(port_file: &std::path::Path) -> Result<u16, ReadPortError> {
        let make_err = |kind| ReadPortError {
            file: port_file.to_path_buf(),
            kind,
        };

        let port = std::fs::read_to_string(port_file)
            .map_err(ReadPortErrorKind::Load)
            .map_err(make_err)?;
        let port = port
            .parse()
            .map_err(ReadPortErrorKind::InvalidText)
            .map_err(make_err)?;
        Ok(port)
    }
    fn build_uri(&self, path: &str) -> String {
        format!("http://{IP_ADDR_LOCAL}:{port}/{path}", port = self.port)
    }
    fn agent() -> ureq::Agent {
        let config = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .build();
        ureq::Agent::from(config)
    }
    fn read_response(
        mut response: http::response::Response<ureq::Body>,
    ) -> Result<String, HttpError> {
        response
            .body_mut()
            .read_to_string()
            .map_err(HttpError::ReadBody)
    }
    pub fn http_get(&mut self, path: &str) -> Result<String, HttpError> {
        let uri = self.build_uri(path);

        let response = Self::agent()
            .get(&uri)
            .call()
            .map_err(|source| HttpError::Send {
                uri: uri.clone(),
                source,
            })
            .and_then(Self::read_response)?;

        tracing::debug!(uri, response);

        Ok(response)
    }
    pub fn http_post<T>(&mut self, path: &str, body: &T) -> Result<String, HttpError>
    where
        T: serde::Serialize,
    {
        let body = serde_json::to_vec(body).map_err(HttpError::SerializeBody)?;

        let uri = self.build_uri(path);

        let response = Self::agent()
            .post(&uri)
            .header("Content-Type", "application/json")
            .send(body)
            .map_err(|source| HttpError::Send {
                uri: uri.clone(),
                source,
            })
            .and_then(Self::read_response)?;

        tracing::debug!(uri, response);

        Ok(response)
    }
}
#[derive(Debug, thiserror::Error)]
#[error("failed to read port file {file}")]
pub struct ReadPortError {
    file: std::path::PathBuf,
    #[source]
    kind: ReadPortErrorKind,
}
#[derive(Debug, thiserror::Error)]
enum ReadPortErrorKind {
    #[error(transparent)]
    Load(std::io::Error),
    #[error(transparent)]
    InvalidText(std::num::ParseIntError),
}

#[derive(Debug, thiserror::Error)]
pub enum HttpError {
    #[error("failed to serialize body")]
    SerializeBody(#[source] serde_json::Error),
    #[error("failed to send request for {uri}")]
    Send { uri: String, source: ureq::Error },
    #[error("failed to read response")]
    ReadBody(#[source] ureq::Error),
}

#[derive(Debug)]
pub struct ReplyPlan {
    pub request_pattern: serde_json::Value,
    pub response: serde_json::Value,
}
type MpscSendResult<T, V = ()> = Result<V, std::sync::mpsc::SendError<T>>;
type MpscSendThread<T, V = ()> = std::thread::JoinHandle<MpscSendResult<T, V>>;
type RespondThread<E> = MpscSendThread<Result<String, E>, Vec<ReplyPlan>>;
impl PipeRunner {
    fn spawn_respond<E>(
        stdin_tx: std::sync::mpsc::SyncSender<Result<String, E>>,
        stdout_rx: std::sync::mpsc::Receiver<OutStr>,
    ) -> (std::sync::mpsc::Sender<ReplyPlan>, RespondThread<E>)
    where
        E: Send + 'static,
    {
        let (respond_tx, respond_rx) = std::sync::mpsc::channel::<ReplyPlan>();

        let respond_thread = std::thread::spawn(move || {
            use std::collections::HashMap;

            let mut responses = HashMap::new();

            let push_plan = |responses: &mut HashMap<serde_json::Value, Vec<serde_json::Value>>,
                             reply_plan: ReplyPlan| {
                let entry = responses.entry(reply_plan.request_pattern).or_default();
                entry.push(reply_plan.response);
            };

            while let Ok(line) = stdout_rx.recv() {
                for reply_plan in respond_rx.try_iter() {
                    push_plan(&mut responses, reply_plan);
                }

                let line: serde_json::Value = match serde_json::from_str(&line) {
                    Ok(v) => v,
                    Err(e) => {
                        tracing::error!(?e, ?line, "invalid json from stdout");
                        continue;
                    }
                };

                tracing::trace!(?responses, ?line);

                if let Some(list) = responses.get_mut(&line)
                    && let Some(response) = list.pop()
                {
                    tracing::debug!(?responses, ?response, "MATCH");

                    stdin_tx.send(Ok(response.to_string()))?;
                } else {
                    tracing::warn!(?line, "No matching response plans");
                }
                if let Some(list) = responses.get(&line)
                    && list.is_empty()
                {
                    responses.remove(&line);
                }
            }
            // drain remaining plans, to report unused entries to thread joiner
            for reply_plan in respond_rx {
                push_plan(&mut responses, reply_plan);
            }
            Ok(responses
                .into_iter()
                .flat_map(|(request_pattern, responses_list)| {
                    responses_list.into_iter().map(move |response| ReplyPlan {
                        request_pattern: request_pattern.clone(),
                        response,
                    })
                })
                .collect())
        });
        (respond_tx, respond_thread)
    }
    fn wait_success_inner(self) -> (WaitSuccessResult, MpscSendResult<StdinMsg, Vec<ReplyPlan>>) {
        const WAIT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

        let Self {
            cmd,
            port: _,
            respond_tx,
            respond_thread,
        } = self;

        drop(respond_tx);
        let wait_result = cmd.wait_success(WAIT_TIMEOUT);

        if let Ok(Ok(output)) = &wait_result {
            eprintln!("{output}");
        }

        // respond_thread guaranteed to end, by two statements above ^^^
        // - drop respond_tx - end of input plans
        // - cmd.wait_success (drops cmd) - end of command output
        let remaining_replies_result = respond_thread.join().expect("panic in respond_thread");
        (wait_result, remaining_replies_result)
    }
}
