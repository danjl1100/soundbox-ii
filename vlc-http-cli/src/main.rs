// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Proof-of-concept for using the [`vlc_http`] crate without an async runtime
//!
//! For the experiment to succeed, this binary crate should be simple and tiny
//! (e.g. main.rs < 200 lines)

use std::str::FromStr as _;
use vlc_http::clap::clap_crate::{self as clap, Parser};

#[derive(clap::Parser, Debug)]
struct GlobalArgs {
    #[clap(flatten)]
    auth: vlc_http::clap::AuthInput,
    /// Print full response text for each request
    #[clap(long)]
    print_responses_http: bool,
    #[clap(long)]
    print_responses: bool,
}

#[derive(clap::Parser, Debug)]
#[clap(no_binary_name = true)]
struct CliArgs {
    #[command(subcommand)]
    action: CliAction,
}

#[derive(clap::Subcommand, Debug)]
enum CliAction {
    Command {
        #[command(subcommand)]
        command: vlc_http::clap::Command,
    },
    Query {
        #[command(subcommand)]
        query: Query,
    },
    Action {},
    #[clap(alias = "exit", alias = "q")]
    Quit,
}

#[derive(clap::Subcommand, Debug)]
enum Query {
    Playlist,
    // TODO
    // Playback,
}

struct Shutdown;

fn main() -> anyhow::Result<()> {
    let GlobalArgs {
        auth,
        print_responses_http,
        print_responses,
    } = GlobalArgs::parse();

    let mut client = Client {
        auth: vlc_http::Auth::new(auth.into())?,
        print_responses_http,
        print_responses,
        client_state: vlc_http::ClientState::new(),
    };

    for line in std::io::stdin().lines() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // NOTE: simplistic whitespace splitting should suffice,
        // as any path/string arguments should be in URL form (percent-encoded)
        let args = line.split_whitespace();
        let command = match CliArgs::try_parse_from(args) {
            Ok(command) => command,
            Err(err) => {
                eprintln!("{err}");
                continue;
            }
        };

        match client.run_action(command.action) {
            Ok(Some(Shutdown)) => break,
            Ok(None) => {}
            Err(err) => {
                eprintln!("{err}");
            }
        }
    }

    Ok(())
}

struct Client {
    auth: vlc_http::Auth,
    print_responses_http: bool,
    print_responses: bool,
    client_state: vlc_http::ClientState,
}
impl Client {
    fn run_action(&mut self, action: CliAction) -> anyhow::Result<Option<Shutdown>> {
        match action {
            CliAction::Command { command } => {
                let endpoint = vlc_http::Command::try_from(command)?.into_endpoint();
                let _response = self.call_endpoint(endpoint);
                Ok(None)
            }
            CliAction::Query {
                query: Query::Playlist,
            } => {
                let action = vlc_http::Action::query_playlist(&self.client_state);

                let result = self.exhaust_pollable(action)?;
                dbg!(result);

                Ok(None)
            }
            CliAction::Action {} => todo!(),
            CliAction::Quit => Ok(Some(Shutdown)),
        }
    }

    fn call_endpoint(&self, endpoint: vlc_http::Endpoint) -> anyhow::Result<vlc_http::Response> {
        let request = endpoint.with_auth(&self.auth).build_http_request();

        let request = {
            let (parts, ()) = request.into_parts();
            ureq::Request::from(parts)
        };

        let response = request.call()?;
        let response_body = response.into_string()?;

        if self.print_responses_http {
            println!("{response_body}");
        }

        let response = vlc_http::Response::from_str(&response_body)?;

        if self.print_responses {
            println!("{response:#?}");
        }

        Ok(response)
    }

    fn exhaust_pollable<T: vlc_http::Pollable>(
        &mut self,
        mut source: T,
    ) -> anyhow::Result<T::Output> {
        const MAX_ITER_COUNT: usize = 100;
        for _ in 0..MAX_ITER_COUNT {
            let endpoint = match source.next_endpoint(&self.client_state) {
                Ok(endpoint) => endpoint,
                Err(result) => return Ok(result),
            };
            let response = self.call_endpoint(endpoint)?;
            self.client_state.update(response);
        }
        anyhow::bail!(
            "exceeded iteration count safety net ({MAX_ITER_COUNT}) for source {source:?}"
        )
    }
}
