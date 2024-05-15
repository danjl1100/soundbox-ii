// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use super::Model;
use clap::Parser as _;
use std::str::FromStr;
use vlc_http::{ClientState, Endpoint, Response};

#[derive(Default)]
pub struct Harness {
    model: Model,
    log: Vec<LogEntry>,
}
#[derive(serde::Serialize)]
pub struct LogEntry(Endpoint, Model);

impl Harness {
    pub fn run_input(input: &str) -> Vec<LogEntry> {
        let mut harness = Self::new();
        let mut client_state = ClientState::new();

        for line in input.lines() {
            let line = line.trim();

            let test_action = match TestInput::try_parse_from(line.split_whitespace()) {
                Ok(test_action) => test_action,
                Err(e) => panic!(
                    "{e}\n{}\ninvalid test action: {line:?}",
                    TestInput::full_help_text()
                ),
            };
            let endpoint = match test_action.action {
                TestAction::Command { command } => match vlc_http::Command::try_from(command) {
                    Ok(endpoint) => endpoint,
                    Err(e) => panic!("invalid command {line:?}: {e}"),
                }
                .into_endpoint(),
            };

            harness.update_for(endpoint, &mut client_state);
        }

        harness.log
    }
    pub fn new() -> Self {
        Self::default()
    }
    pub fn update_for(&mut self, endpoint: Endpoint, target: &mut ClientState) {
        let endpoint_str = endpoint.get_path_and_query();

        let response_str = self.model.request(endpoint_str);
        let response = match Response::from_str(&response_str) {
            Ok(response) => response,
            Err(e) => panic!("invalid response from model {response_str:?}: {e}"),
        };

        target.update(response.clone());

        let log_entry = LogEntry(endpoint, self.model.clone());
        self.log.push(log_entry);
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
}
impl TestInput {
    fn full_help_text() -> impl std::fmt::Display {
        struct R;
        impl std::fmt::Display for R {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(
                    f,
                    "{}",
                    TestInput::try_parse_from(["command", "--help"]).expect_err("help errors"),
                )
            }
        }
        R
    }
}
