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

use clap::{ArgEnum, Parser};
use std::io::{stdin, BufRead};

use arg_split::ArgSplit;
use sequencer::{
    sources::{Beet, FileLines},
    DebugItemSource, Error, Sequencer,
};

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
    /// Add a new node for items or fanning-out to child nodes
    Add {
        /// Path of the parent for the new node (use "." for the root node)
        parent_path: String,
        /// Filename source, for terminal nodes only (optional)
        items_filter: Vec<String>,
    },
    /// Update items for a node
    Update {
        /// Path of the target node to update (optional, default is all nodes)
        path: Option<String>,
    },
    /// Remove a node
    Remove {
        /// Id of the target node to delete
        id: String,
        //TODO
        // recursive: bool,
    },
    /// Print the next item(s)
    #[clap(alias("n"))]
    Next {
        /// Number of items to print
        count: Option<usize>,
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
    sequencer: TypedSequencer,
}
shared::wrapper_enum! {
    enum TypedSequencer {
        DebugItem(Sequencer<DebugItemSource, String>),
        FileLines(Sequencer<FileLines, String>),
        Beet(Sequencer<Beet, Vec<String>>),
    }
}
impl Default for TypedSequencer {
    fn default() -> Self {
        TypedSequencer::DebugItem(Sequencer::default())
    }
}

macro_rules! match_seq {
    ( $seq:expr, $bound:ident => $call:expr ) => {
        match $seq {
            TypedSequencer::DebugItem($bound) => $call,
            TypedSequencer::FileLines($bound) => $call,
            TypedSequencer::Beet($bound) => $call,
        }
    };
}

impl Cli {
    fn new(sequencer: TypedSequencer) -> Self {
        Self { sequencer }
    }
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
                match_seq!(&self.sequencer, inner => println!("{inner}"));
            }
            Command::Add {
                parent_path,
                items_filter,
            } => {
                let joined = items_filter.join(" ");
                match &mut self.sequencer {
                    TypedSequencer::DebugItem(inner) => {
                        let node_path = if items_filter.is_empty() {
                            inner.add_node(&parent_path, joined)
                        } else {
                            inner.add_terminal_node(&parent_path, joined)
                        }?;
                        println!("added node {node_path}");
                    }
                    TypedSequencer::FileLines(inner) => {
                        let node_path = if items_filter.is_empty() {
                            inner.add_node(&parent_path, joined)
                        } else {
                            inner.add_terminal_node(&parent_path, joined)
                        }?;
                        println!("added node {node_path}");
                    }
                    TypedSequencer::Beet(inner) => {
                        let node_path = if items_filter.is_empty() {
                            inner.add_node(&parent_path, items_filter)
                        } else {
                            inner.add_terminal_node(&parent_path, items_filter)
                        }?;
                        println!("added node {node_path}");
                    }
                }
            }
            Command::Update { path } => {
                let path = path.as_ref().map_or(".", |p| p);
                let path = match_seq!(&mut self.sequencer, inner => inner.update_node(path)?);
                println!("updated nodes under path {path}");
            }
            Command::Remove { id } => {
                match_seq!(&mut self.sequencer, inner => {
                    let removed = inner.remove_node(&id)?;
                    let (weight, info) = removed;
                    println!("removed node {id}: weight = {weight}, {info:#?}");
                });
            }
            Command::Next { count } => {
                for _ in 0..count.unwrap_or(1) {
                    let popped = match_seq!(&mut self.sequencer, inner => inner.pop_next());
                    if let Some(item) = popped {
                        println!("Item {item:?}");
                    } else {
                        println!("No items remaining");
                        break;
                    }
                }
            }
        }
        Ok(())
    }
}

#[derive(Parser)]
struct MainArgs {
    /// Source for the item source
    #[clap(arg_enum)]
    source: Option<ItemSourceType>,
    /// Command to use for the [`Beet`] item source type
    beet_cmd: Option<String>,
}
#[derive(Clone, ArgEnum)]
enum ItemSourceType {
    Debug,
    FileLines,
    Beet,
}
fn main() -> Result<(), std::io::Error> {
    eprint!("{}", COMMAND_NAME);
    eprintln!("{}", shared::license::WELCOME);

    let args = MainArgs::parse();
    let sequencer = match args.source.unwrap_or(ItemSourceType::FileLines) {
        ItemSourceType::Debug => TypedSequencer::DebugItem(Sequencer::new(DebugItemSource)),
        ItemSourceType::FileLines => {
            TypedSequencer::FileLines(Sequencer::new(FileLines::new(".".into())?))
        }
        ItemSourceType::Beet => {
            if let Some(beet_cmd) = args.beet_cmd {
                TypedSequencer::Beet(Sequencer::new(Beet::new(beet_cmd)?))
            } else {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "missing beet_cmd",
                ));
            }
        }
    };
    Cli::new(sequencer).exec_lines(stdin().lock())
}
