// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Proof-of-concept for using the [`vlc_http`] crate without an async runtime
//!
//! For the experiment to succeed, this binary crate should be simple and tiny
//! (e.g. main.rs ~200 lines, or so)

use vlc_http::{
    clap::clap_crate::{self as clap, Parser},
    http_runner::ureq::HttpRunner,
    sync::EndpointRequestor,
};

#[derive(clap::Parser, Debug)]
struct GlobalArgs {
    #[clap(flatten)]
    auth: vlc_http::clap::AuthInput,
    /// Print full response text for each request
    #[clap(long)]
    print_responses_http: bool,
    #[clap(long)]
    print_responses: bool,
    #[clap(subcommand)]
    oneshot_action: Option<OneshotAction>,
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
    Action {
        #[command(subcommand)]
        action: vlc_http::clap::Action,
    },
    #[clap(alias = "exit", alias = "q")]
    Quit,
}
#[derive(clap::Subcommand, Debug)]
enum OneshotAction {
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
}
impl From<OneshotAction> for CliAction {
    fn from(value: OneshotAction) -> Self {
        match value {
            OneshotAction::Command { command } => Self::Command { command },
            OneshotAction::Query { query } => Self::Query { query },
            OneshotAction::Action { action } => Self::Action { action },
        }
    }
}

#[derive(clap::Subcommand, Debug)]
enum Query {
    Playlist,
    Playback,
    PlaylistSet(vlc_http::clap::PlaylistSetQueryMatched),
}

struct Shutdown;

fn main() -> eyre::Result<()> {
    let GlobalArgs {
        auth,
        print_responses_http,
        print_responses,
        oneshot_action,
    } = GlobalArgs::parse();

    let auth = vlc_http::Auth::new(auth.into())?;

    let mut client = Client {
        runner: HttpRunner::new(auth),
        client_state: vlc_http::ClientState::new(),
    };

    if print_responses_http {
        client
            .runner
            .set_observe_responses_str(Box::new(|response: &str| {
                println!("{response}");
            }));
    }
    if print_responses {
        client
            .runner
            .set_observe_responses(Box::new(|response: &vlc_http::Response| {
                println!("{response:#?}");
            }));
    }

    if let Some(action) = oneshot_action {
        client.run_action(action.into())?;
        Ok(())
    } else {
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
}

struct Client {
    runner: HttpRunner,
    client_state: vlc_http::ClientState,
}
impl Client {
    fn run_action(&mut self, action: CliAction) -> eyre::Result<Option<Shutdown>> {
        match action {
            CliAction::Command { command } => {
                let endpoint = vlc_http::Command::try_from(command)?.into_endpoint();
                let _response = self.runner.request(endpoint);
                Ok(None)
            }
            CliAction::Query {
                query: Query::Playlist,
            } => {
                let result = self.complete_plan(vlc_http::Action::query_playlist(
                    self.client_state.get_ref(),
                ))?;
                dbg!(result);

                Ok(None)
            }
            CliAction::Query {
                query: Query::Playback,
            } => {
                let result = self.complete_plan(vlc_http::Action::query_playback(
                    self.client_state.get_ref(),
                ))?;
                dbg!(result);

                Ok(None)
            }
            CliAction::Query {
                query: Query::PlaylistSet(target),
            } => {
                let result = self.complete_plan(vlc_http::Action::set_playlist_query_matched(
                    target.into(),
                    self.client_state.get_ref(),
                ))?;
                dbg!(result);

                Ok(None)
            }
            CliAction::Action { action } => {
                self.complete_plan(
                    vlc_http::Action::from(action).into_plan(self.client_state.get_ref()),
                )?;

                Ok(None)
            }
            CliAction::Quit => Ok(Some(Shutdown)),
        }
    }

    fn complete_plan<T>(&mut self, plan: T) -> eyre::Result<T::Output<'_>>
    where
        T: vlc_http::Plan,
        eyre::Report: From<vlc_http::sync::Error<T, vlc_http::http_runner::ureq::Error>>,
    {
        const MAX_ITER_COUNT: usize = 100;
        let (output, _seq) = vlc_http::sync::complete_plan(
            plan,
            &mut self.client_state,
            &mut self.runner,
            MAX_ITER_COUNT,
        )?;
        Ok(output)
    }
}
