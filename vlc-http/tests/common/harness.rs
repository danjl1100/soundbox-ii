// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use super::Model;
use clap::Parser as _;
use std::{collections::VecDeque, num::NonZeroU32};
use vlc_http::{action::Poll, ClientState, Endpoint, Pollable};

pub fn run_input(input: &str) -> Vec<LogEntry> {
    println!("============= run input =============");

    let mut runner = Runner::default();

    for line in input.lines() {
        let line = line.trim();

        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let test_action = match TestInput::try_parse_from(line.split_whitespace()) {
            Ok(test_action) => test_action,
            Err(e) => panic!(
                "{e}\n--- help text\n{}\ninvalid test input: {line:?}",
                TestInput::full_help_text(line.split_whitespace().map(str::to_owned).collect()),
            ),
        };

        runner.run_test_action(test_action, line);
    }

    runner.into_log()
}

#[derive(Default)]
struct Runner {
    model_logger: ModelLogger,
    client_state: ClientState,
    action_step_limit: Option<u32>,
    action_ignored_endpoints: VecDeque<Endpoint>,
    action_pending: Option<vlc_http::action::ActionPollable>,
}

impl Runner {
    fn run_test_action(&mut self, test_action: TestInput, line: &str) {
        match test_action.action {
            TestAction::Command { command } => {
                let endpoint = match vlc_http::Command::try_from(command) {
                    Ok(endpoint) => endpoint,
                    Err(e) => panic!("invalid command {line:?}: {e}"),
                }
                .into_endpoint();

                self.run_endpoint(endpoint);
            }
            TestAction::Query {
                query: Query::Art { item_id },
            } => {
                let endpoint = vlc_http::Command::art_endpoint(&item_id);
                self.run_endpoint(endpoint);
            }
            TestAction::Action { action } => {
                if let Some(action_pending) = &self.action_pending {
                    // TODO add error-handling, so that we can print the full log on all errors
                    panic!("invalid command {line:?}: cannot start action when one is already pending: {action_pending:#?}");
                }
                self.action_pending =
                    Some(vlc_http::Action::from(action).pollable(&self.client_state));
                self.run_pending_action(line);
            }
            TestAction::Harness { override_command } => match override_command {
                OverrideCommand::InitItems { items } => {
                    self.model_logger
                        .edit_model(|model| model.initialize_items(items));
                }
                OverrideCommand::ActionStepLimit { step_count } => {
                    self.action_step_limit = Some(step_count);
                }
                OverrideCommand::ActionClearLimit => {
                    self.action_step_limit = None;
                }
                OverrideCommand::ActionIgnorePush { push_count } => {
                    let push_count = push_count.map_or(1, NonZeroU32::get);
                    self.action_pending = {
                        let mut pollable = self.take_action_pending(line);
                        let mut iter = 0;
                        loop {
                            let Some(endpoint) = self.next_endpoint_from(&mut pollable, line)
                            else {
                                panic!("invalid command {line:?}: action returned None (completed) so cannot ignore (iter {iter})")
                            };

                            // log output from pollable
                            self.model_logger.log_endpoint_only(endpoint.clone());
                            // queue for delayed use
                            self.action_ignored_endpoints.push_back(endpoint);

                            iter += 1;
                            if iter >= push_count {
                                break Some(pollable);
                            }
                        }
                    };
                }
                OverrideCommand::ActionIgnorePop { pop_count } => {
                    let pop_count = pop_count.map_or(1, NonZeroU32::get);
                    for iter in 0..pop_count {
                        let Some(endpoint) = self.action_ignored_endpoints.pop_front() else {
                            panic!("invalid state for {line:?}: no ignored endpoint found for ActionApplyIgnored (iter {iter})")
                        };
                        self.run_endpoint(endpoint);
                    }
                }
                OverrideCommand::ActionResume => self.run_pending_action(line),
            },
        }
    }
    fn run_endpoint(&mut self, endpoint: Endpoint) {
        self.model_logger
            .update_for(endpoint, &mut self.client_state);
    }
    fn take_action_pending(&mut self, line: &str) -> vlc_http::action::ActionPollable {
        let Some(pollable) = self.action_pending.take() else {
            panic!("invalid state for {line:?}: no action_pending to use")
        };
        pollable
    }
    fn next_endpoint_from(
        &self,
        pollable: &mut vlc_http::action::ActionPollable,
        line: &str,
    ) -> Option<Endpoint> {
        match pollable.next(&self.client_state) {
            Ok(Poll::Need(endpoint)) => Some(endpoint),
            Ok(Poll::Done(())) => None,
            Err(vlc_http::action::Error::InvalidClientInstance(_)) => {
                panic!("invalidate state for {line:?}: non-singleton client_state")
            }
        }
    }
    fn run_pending_action(&mut self, line: &str) {
        self.action_pending = {
            let mut pollable = self.take_action_pending(line);
            let mut iter_count = 0;
            loop {
                match self.action_step_limit {
                    // NOTE: allow cutoff at "step_limit = 0" for testing the harness
                    Some(step_limit) if iter_count >= step_limit => {
                        break Some(pollable);
                    }
                    _ => {}
                }

                let Some(endpoint) = self.next_endpoint_from(&mut pollable, line) else {
                    break None;
                };
                self.run_endpoint(endpoint);

                iter_count += 1;
            }
        };
    }

    fn into_log(self) -> Vec<LogEntry> {
        let Self {
            model_logger,
            client_state: _,
            action_step_limit: _,
            action_ignored_endpoints,
            action_pending,
        } = self;
        let log = model_logger.into_log();
        let log_json = || serde_json::to_string_pretty(&log).expect("json serialize log");

        if let Some(action_pending) = action_pending {
            let log_str = log_json();
            panic!("FAIL log entries: {log_str}\nFAIL ended while still pending action: {action_pending:#?}");
        }

        if !action_ignored_endpoints.is_empty() {
            let log_str = log_json();
            panic!("FAIL log entries: {log_str}\nFAIL ended while still pending ignored endpoints: {action_ignored_endpoints:#?}");
        }

        log
    }
}

use model_logger::{LogEntry, ModelLogger};
mod model_logger {
    use super::Model;
    use std::str::FromStr;
    use vlc_http::{ClientState, Endpoint, Response};

    #[derive(Debug, PartialEq, Eq, serde::Serialize)]
    pub enum LogEntry {
        #[serde(rename = "LogEntry")]
        Endpoint(Endpoint, Model),
        #[serde(rename = "Harness")]
        HarnessEndpoint(Endpoint),
        #[serde(rename = "Harness")]
        HarnessModel(Model),
    }

    #[derive(Default)]
    pub(super) struct ModelLogger {
        model: Model,
        log: Vec<LogEntry>,
    }
    impl ModelLogger {
        pub fn update_for(&mut self, endpoint: Endpoint, target: &mut ClientState) {
            const MAX_LOG_COUNT: usize = 50;
            const MAX_REPEAT_COUNT: usize = 10;

            println!("---- {endpoint:?}");

            let endpoint_str = endpoint.get_path_and_query();

            let response_str = self.model.request(endpoint_str);
            let response = match Response::from_str(&response_str) {
                Ok(response) => response,
                Err(e) => panic!("invalid response from model {response_str:?}: {e}"),
            };

            target.update(response.clone());

            let log_entry = LogEntry::Endpoint(endpoint, self.model.clone());

            if !self.log.is_empty()
                && self
                    .log
                    .iter()
                    .rev()
                    .take(MAX_REPEAT_COUNT)
                    .all(|log_last| *log_last == log_entry)
            {
                let log_str = serde_json::to_string_pretty(&self.log).expect("json serialize log");
                panic!("FAIL log entries: {log_str}\nFAIL Cycle detected, duplicated log entry {log_entry:#?}");
            }

            self.log.push(log_entry);

            assert!(self.log.len() <= MAX_LOG_COUNT, "Log length is too long");
        }
        pub fn log_endpoint_only(&mut self, endpoint: Endpoint) {
            self.log.push(LogEntry::HarnessEndpoint(endpoint));
        }
        pub fn edit_model<R>(&mut self, modify_fn: impl FnOnce(&mut Model) -> R) -> R {
            let result = modify_fn(&mut self.model);
            self.log.push(LogEntry::HarnessModel(self.model.clone()));
            result
        }
        pub fn into_log(self) -> Vec<LogEntry> {
            self.log
        }
    }
}

#[derive(clap::Parser, Debug)]
#[clap(no_binary_name = true)]
struct TestInput {
    #[command(subcommand)]
    action: TestAction,
}
#[derive(clap::Subcommand, Debug)]
enum TestAction {
    Command {
        #[command(subcommand)]
        command: vlc_http::clap::Command,
    },
    Query {
        #[command(subcommand)]
        query: Query,
    },
    Action {
        #[command(subcommand)]
        action: vlc_http::clap::Action,
    },
    Harness {
        #[command(subcommand)]
        override_command: OverrideCommand,
    },
}
#[derive(clap::Subcommand, Debug)]
enum Query {
    Art { item_id: String },
}
/// Overrides to simulate anomalies in VLC server behavior
#[derive(clap::Subcommand, Debug)]
enum OverrideCommand {
    /// One-time initialization of the items (to avoid tedious setup with "playlist-add" commands)
    #[clap(alias = "items")]
    InitItems { items: Vec<String> },
    /// Pauses future actions after the specified number of steps (for use in `ActionResume`)
    ///
    /// Errors if the test ends while an action is paused
    ActionStepLimit { step_count: u32 },
    /// Runs future actions to completion (for use in `ActionResume` and future actions)
    ActionClearLimit,
    /// Poll one endpoint from the current action, but do not act on the endpoint
    ///
    /// Stores the unused endpoint in a queue for use in `ActionApplyIgnored`
    ActionIgnorePush { push_count: Option<NonZeroU32> },
    /// Applies the first ignored endpoint from the `ActionStepIgnore` queue
    ///
    /// Errors if the queue is empty (cannot apply more than were actually ignored)
    ActionIgnorePop { pop_count: Option<NonZeroU32> },
    /// Resume the previous action (errors if none are pending)
    ActionResume,
}
impl TestInput {
    fn full_help_text(mut input: Vec<String>) -> impl std::fmt::Display {
        struct R(Vec<String>);
        impl std::fmt::Display for R {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(
                    f,
                    "{}",
                    TestInput::try_parse_from(&self.0).expect_err("help errors"),
                )
            }
        }
        input.push("--help".to_owned());
        R(input)
    }
}
