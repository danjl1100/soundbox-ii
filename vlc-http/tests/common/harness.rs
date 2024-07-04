// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use super::Model;
use clap::Parser as _;
use std::str::FromStr;
use vlc_http::{action::Poll, ClientState, Endpoint, Pollable, Response};

#[derive(Default)]
pub struct Harness {
    model: Model,
    log: Vec<LogEntry>,
}
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
pub enum LogEntry {
    #[serde(rename = "LogEntry")]
    Endpoint(Endpoint, Model),
    #[serde(rename = "Harness")]
    HarnessModel(Model),
}

impl Harness {
    pub fn run_input(input: &str) -> Vec<LogEntry> {
        println!("============= run input =============");

        let mut harness = Self::new();
        let mut client_state = ClientState::new();

        for line in input.lines() {
            let line = line.trim();

            let test_action = match TestInput::try_parse_from(line.split_whitespace()) {
                Ok(test_action) => test_action,
                Err(e) => panic!(
                    "{e}\n--- help text\n{}\ninvalid test input: {line:?}",
                    TestInput::full_help_text(line.split_whitespace().map(str::to_owned).collect()),
                ),
            };
            match test_action.action {
                TestAction::Command { command } => {
                    let endpoint = match vlc_http::Command::try_from(command) {
                        Ok(endpoint) => endpoint,
                        Err(e) => panic!("invalid command {line:?}: {e}"),
                    }
                    .into_endpoint();

                    harness.update_for(endpoint, &mut client_state);
                }
                TestAction::Query {
                    query: Query::Art { item_id },
                } => {
                    let endpoint = vlc_http::Command::art_endpoint(&item_id);

                    harness.update_for(endpoint, &mut client_state);
                }
                TestAction::Action { action } => {
                    let mut pollable = vlc_http::Action::from(action).pollable(&client_state);
                    while let Poll::Need(endpoint) = pollable
                        .next(&client_state)
                        .expect("singleton client_state")
                    {
                        println!("---- {endpoint:?}");
                        harness.update_for(endpoint, &mut client_state);
                    }
                }
                TestAction::Harness { init_command } => harness.init_command(init_command),
            }
        }

        harness.log
    }
    pub fn new() -> Self {
        Self::default()
    }
    pub fn update_for(&mut self, endpoint: Endpoint, target: &mut ClientState) {
        const MAX_LOG_COUNT: usize = 50;

        let endpoint_str = endpoint.get_path_and_query();

        let response_str = self.model.request(endpoint_str);
        let response = match Response::from_str(&response_str) {
            Ok(response) => response,
            Err(e) => panic!("invalid response from model {response_str:?}: {e}"),
        };

        target.update(response.clone());

        let log_entry = LogEntry::Endpoint(endpoint, self.model.clone());

        if let Some(log_last) = self.log.last() {
            assert!(
                *log_last != log_entry,
                "Cycle detected, duplicated log entry {log_last:#?}"
            );
        }

        self.log.push(log_entry);

        assert!(self.log.len() <= MAX_LOG_COUNT, "Log length is too long");
    }
    fn init_command(&mut self, init_command: InitCommand) {
        match init_command {
            InitCommand::Items { items } => self.model.initialize_items(items),
        }
        self.log.push(LogEntry::HarnessModel(self.model.clone()));
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
        init_command: InitCommand,
    },
}
#[derive(clap::Subcommand, Debug)]
enum Query {
    Art { item_id: String },
}
#[derive(clap::Subcommand, Debug)]
enum InitCommand {
    Items { items: Vec<String> },
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
