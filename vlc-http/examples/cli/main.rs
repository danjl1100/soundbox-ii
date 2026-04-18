// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Proof-of-concept for using the [`vlc_http`] crate without an async runtime
//!
//! For the experiment to succeed, this binary crate should be simple and tiny
//! (e.g. main.rs ~200 lines, or so)

use arg_util::ConfigFileWrite;
use eyre::Context as _;
use vlc_http::sync::EndpointRequestor;
use vlc_http_auth::AuthInput;
use vlc_http_auth_clap::clap_crate::{self as clap, Parser};
use vlc_http_ureq::HttpRunner;

#[derive(clap::Parser, Debug)]
struct GlobalArgs {
    #[clap(flatten)]
    auth_args: vlc_http_auth_clap::ClapAuthInputOptional,
    /// TOML file containing VLC authentication
    // NOTE: Even though the scope of this example is VLC only,
    // still include the `vlc-` prefix to show it's related to the other `vlc-` long args above
    #[clap(long)]
    vlc_auth_file: Option<std::path::PathBuf>,
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
        #[command(flatten)]
        command: vlc_http_cmd_clap::command::ClapCommand,
    },
    Query {
        #[command(subcommand)]
        query: Query,
    },
    Action {
        #[command(subcommand)]
        action: vlc_http_cmd_clap::goal::ClapChange,
    },
    #[clap(alias = "exit", alias = "q")]
    Quit,
}
#[derive(clap::Subcommand, Debug)]
enum OneshotAction {
    Command {
        #[command(flatten)]
        command: vlc_http_cmd_clap::command::ClapCommand,
    },
    Query {
        #[command(subcommand)]
        query: Query,
    },
    Action {
        #[command(subcommand)]
        action: vlc_http_cmd_clap::goal::ClapChange,
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
    PlaylistSet(vlc_http_cmd_clap::goal::ClapPlaylistSetQueryMatched),
}

struct Shutdown;

fn main() -> eyre::Result<()> {
    let GlobalArgs {
        auth_args,
        vlc_auth_file,
        print_responses_http,
        print_responses,
        oneshot_action,
    } = GlobalArgs::parse();

    let auth_input = get_auth_with_file(auth_args.into(), vlc_auth_file)?;
    let auth = vlc_http::Auth::new(auth_input)?;

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

fn get_auth_with_file(
    auth_args: vlc_http_auth::optional::AuthInputOptional,
    auth_file: Option<std::path::PathBuf>,
) -> eyre::Result<AuthInput> {
    let (partial_args, auth_file) = match (auth_args.try_into(), auth_file) {
        (Ok(complete_args), _) => {
            // args are complete, no need to check the file
            return Ok(complete_args);
        }
        (Err(e), Some(auth_file)) => (e.into_inner(), auth_file),
        (Err(e), None) => eyre::bail!(e),
    };

    // check for more args in the file
    let result: Result<AuthInput, _> = arg_util::config_file::ConfigFileOpen::open(&auth_file);
    let auth_file = match result {
        Ok(auth) => auth,
        Err(e) if e.is_missing_file() => {
            let template_file = AuthInput::sample_for_templates()
                .write_template_for_file(&auth_file)
                .with_context(|| {
                    format!(
                        "file not found ({auth_file}), then failed to create VLC HTTP auth template file",
                        auth_file = auth_file.display(),
                    )
                })?;
            eyre::bail!(
                "file not found ({auth_file}), created VLC HTTP auth template file at: {template_file}",
                auth_file = auth_file.display(),
                template_file = template_file.display(),
            )
        }
        Err(e) => Err(e)?,
    };
    let auth = partial_args.unwrap_or(auth_file);
    Ok(auth)
}

struct Client {
    runner: HttpRunner,
    client_state: vlc_http::ClientState,
}
impl Client {
    fn run_action(&mut self, action: CliAction) -> eyre::Result<Option<Shutdown>> {
        match action {
            CliAction::Command { command } => {
                let command = vlc_http::Command::try_from(command)?;
                let _response = self.runner.request(command.into());
                Ok(None)
            }
            CliAction::Query {
                query: Query::Playlist,
            } => {
                let result = self.complete_plan(self.client_state.build_plan().query_playlist())?;
                dbg!(result);

                Ok(None)
            }
            CliAction::Query {
                query: Query::Playback,
            } => {
                let result = self.complete_plan(self.client_state.build_plan().query_playback())?;
                dbg!(result);

                Ok(None)
            }
            CliAction::Query {
                query: Query::PlaylistSet(target),
            } => {
                let result = self.complete_plan(
                    self.client_state
                        .build_plan()
                        .set_playlist_and_query_matched(target.into()),
                )?;
                dbg!(result);

                Ok(None)
            }
            CliAction::Action { action } => {
                self.complete_plan(
                    self.client_state
                        .build_plan()
                        .apply(vlc_http::Change::from(action)),
                )?;

                Ok(None)
            }
            CliAction::Quit => Ok(Some(Shutdown)),
        }
    }

    fn complete_plan<T>(&mut self, plan: T) -> eyre::Result<T::Output<'_>>
    where
        T: vlc_http::Plan,
        eyre::Report: From<vlc_http::sync::Error<T, vlc_http_ureq::Error>>,
    {
        const MAX_ITER_COUNT: usize = 100;
        let output = vlc_http::sync::complete_plan(
            plan,
            &mut self.client_state,
            &mut self.runner,
            MAX_ITER_COUNT,
        )?;
        Ok(output)
    }
}
