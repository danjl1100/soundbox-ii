// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Binary for running [`Sequencer`] interactively

// TODO: only while building
#![allow(dead_code)]
// teach me
#![deny(clippy::pedantic)]
// no unsafe
#![forbid(unsafe_code)]
// no unwrap
#![deny(clippy::unwrap_used)]
// no panic
#![deny(clippy::panic)]
// docs!
#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

use clap::Parser;
use std::io::{stdin, BufRead};

use arg_split::ArgSplit;
use sequencer::{DebugItemSource, Error, Sequencer};

const COMMAND_NAME: &str = "sequencer";

#[derive(Parser, Debug)]
#[clap(no_binary_name = true)]
struct Args {
    #[clap(subcommand)]
    command: Option<Command>,
}
impl TryFrom<&String> for Args {
    type Error = clap::Error;
    fn try_from(line: &String) -> Result<Self, clap::Error> {
        let line_parts = ArgSplit::split(line);
        Self::try_parse_from(line_parts)
    }
}

/// Cli Commands
#[derive(Parser, Debug)]
pub enum Command {
    /// Quit the interactive shell (alternative to Ctrl-D, EOF)
    #[clap(alias("q"), alias("exit"))]
    Quit,
    /// Show license snippets
    Show {
        /// The license snippet to show
        #[clap(subcommand)]
        license: ShowCopyingLicenseType,
    },
    /// Print the current sequencer-nodes state
    Print,
    /// Add a new node for fanning-out to child nodes
    Add {
        /// Path of the parent for the new node (use "." for the root node)
        parent_path: String,
        /// Filename source, for terminal nodes only (optional)
        filename: Option<String>,
        /// Filter value (optional)
        filter: Option<String>,
    },
    /// Remove a node
    Remove {
        /// Id of the target node to delete
        id: String,
        //TODO
        // recursive: bool,
    },
}
/// Types of License snippets available to show
#[derive(clap::Subcommand, Debug)]
pub enum ShowCopyingLicenseType {
    /// Show warranty details
    #[clap(alias("w"))]
    Warranty,
    /// Show conditions for redistribution
    #[clap(alias("c"))]
    Copying,
}

#[derive(Default)]
struct Cli {
    sequencer: Sequencer<DebugItemSource>,
}
impl Cli {
    fn exec_lines<T>(&mut self, input: T) -> Result<(), std::io::Error>
    where
        T: BufRead,
    {
        for line in input.lines() {
            let line = line?;
            match Args::try_from(&line) {
                Ok(Args {
                    command: Some(Command::Quit),
                }) => return Ok(()),
                Ok(Args { command: Some(cmd) }) => {
                    let result = self.exec_command(cmd);
                    if let Err(e) = result {
                        match e {
                            Error::Message(message) => eprintln!("ERROR: {message}"),
                            e => eprintln!("ERROR: {e:?}"),
                        }
                    }
                }
                Ok(Args { command: None }) => continue,
                Err(clap_err) => {
                    eprintln!("{clap_err}");
                    continue;
                }
            }
        }
        eprintln!("<<STDIN EOF>>");
        Ok(())
    }
    fn exec_command(&mut self, command: Command) -> Result<(), Error> {
        match command {
            Command::Quit => {}
            Command::Show { license } => match license {
                ShowCopyingLicenseType::Warranty => {
                    eprintln!("{}", shared::license::WARRANTY);
                }
                ShowCopyingLicenseType::Copying => {
                    eprintln!("{}", shared::license::REDISTRIBUTION);
                }
            },
            Command::Print => {
                println!("{}", self.sequencer);
            }
            Command::Add {
                parent_path,
                filename,
                filter,
            } => {
                let node_path = if let Some(filename) = filename {
                    self.sequencer.add_terminal_node(&parent_path, filename)?
                } else {
                    self.sequencer
                        .add_node(&parent_path, filter.unwrap_or_default())?
                };
                println!("added node {node_path}");
            }
            Command::Remove { id } => {
                let removed = self.sequencer.remove_node(&id)?;
                let (weight, info) = removed;
                println!("removed node {id}: weight = {weight}, {info:#?}");
            }
        }
        Ok(())
    }
}

fn main() -> Result<(), std::io::Error> {
    eprint!("{}", COMMAND_NAME);
    eprintln!("{}", shared::license::WELCOME);

    Cli::default().exec_lines(stdin().lock())
}
