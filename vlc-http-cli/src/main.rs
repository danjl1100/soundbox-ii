// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Proof-of-concept for using the [`vlc_http`] crate without an async runtime
//!
//! For the experiment to succeed, this binary crate should be simple and tiny
//! (e.g. main.rs < 200 lines)

use vlc_http::clap::clap_crate::{self as clap, Parser};

#[derive(clap::Parser, Debug)]
struct GlobalArgs {
    #[clap(flatten)]
    auth: vlc_http::clap::AuthInput,
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
    Action {},
    #[clap(alias = "exit", alias = "q")]
    Quit,
}

struct Shutdown;

fn main() -> anyhow::Result<()> {
    let GlobalArgs { auth } = GlobalArgs::parse();
    let auth = vlc_http::Auth::new(auth.into())?;

    for line in std::io::stdin().lines() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let args = arg_util::ArgSplit::split_into_owned(line);
        let command = match CliArgs::try_parse_from(args) {
            Ok(command) => command,
            Err(err) => {
                eprintln!("{err}");
                continue;
            }
        };

        match run_action(command.action, &auth) {
            Ok(Some(Shutdown)) => break,
            Ok(None) => {}
            Err(err) => {
                eprintln!("{err}");
            }
        }
    }

    Ok(())
}

fn run_action(action: CliAction, auth: &vlc_http::Auth) -> anyhow::Result<Option<Shutdown>> {
    match action {
        CliAction::Command { command } => {
            let endpoint = vlc_http::Command::try_from(command)?.into_endpoint();
            let request = endpoint.with_auth(auth).build_http_request();

            let request = {
                let (parts, ()) = request.into_parts();
                ureq::Request::from(parts)
            };

            request.call()?;

            Ok(None)
        }
        CliAction::Action {} => todo!(),
        CliAction::Quit => Ok(Some(Shutdown)),
    }
}