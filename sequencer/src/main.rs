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
use q_filter_tree::{OrderType, Weight};
use std::{
    fs::File,
    io::{stdin, BufRead, BufReader},
};

use arg_split::ArgSplit;
use sequencer::{
    sources::{Beet, FileLines, FolderListing},
    DebugItemSource, Error, Sequencer,
};

const COMMAND_NAME: &str = "sequencer";

#[derive(Parser, Debug)]
#[clap(no_binary_name = true)]
struct Args {
    #[clap(subcommand)]
    command: Option<Command>,
}
impl TryFrom<&str> for Args {
    type Error = clap::Error;
    fn try_from(line: &str) -> Result<Self, clap::Error> {
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
    /// Set the filter for the specified node
    SetFilter {
        /// Path of the node to modify
        path: String,
        /// New filter value
        items_filter: Vec<String>,
    },
    /// Set the weight for the specified node or item
    SetWeight {
        /// Path of the node to modify
        path: String,
        /// Index of the item to modify (for terminal nodes only)
        #[clap(long)]
        item_index: Option<usize>,
        /// New weight value
        weight: Weight,
    },
    /// Set the order type for the specified node
    SetOrderType {
        /// Path of the node to modify
        path: String,
        /// Method of ordering
        #[clap(subcommand)]
        order_type: OrderType,
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

struct Cli {
    sequencer: TypedSequencer,
    params: Parameters,
}
struct Parameters {
    /// Slience non-error output that is not explicitly requested
    quiet: bool,
    /// Terminates on the first error encountered (implied for `--script` mode)
    fatal: bool,
}
impl Parameters {
    fn output(&self, fmt_args: std::fmt::Arguments) {
        if !self.quiet {
            println!("{fmt_args}");
        }
    }
}
shared::wrapper_enum! {
    enum TypedSequencer {
        DebugItem(Sequencer<DebugItemSource, String>),
        FileLines(Sequencer<FileLines, String>),
        FolderListing(Sequencer<FolderListing, String>),
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
            TypedSequencer::FolderListing($bound) => $call,
            TypedSequencer::Beet($bound) => $call,
        }
    };
}

impl Cli {
    const COMMENT: &'static str = "#";
    fn new(sequencer: TypedSequencer, params: Parameters) -> Self {
        Self { sequencer, params }
    }
    fn exec_lines<T>(&mut self, input: T) -> Result<(), MainError>
    where
        T: BufRead,
    {
        for (line_number, line) in input.lines().enumerate() {
            let line = line?;
            match self.exec_line(&line) {
                Ok(Some(shared::Shutdown)) => {
                    self.output(format_args!("exited cleanly"));
                    return Ok(());
                }
                Err(()) if self.params.fatal => {
                    let line_number = line_number + 1; // human-readable one-based counting
                    Err(format!("failed on line {line_number}: {line:?}"))?;
                }
                Ok(None) | Err(()) => {}
            }
        }
        self.output(format_args!("<<EOF>>"));
        Ok(())
    }
    fn exec_line(&mut self, line: &str) -> Result<Option<shared::Shutdown>, ()> {
        if line.trim_start().starts_with(Self::COMMENT) {
            Ok(None)
        } else {
            match Args::try_from(line) {
                Ok(Args {
                    command: Some(Command::Quit),
                }) => Ok(Some(shared::Shutdown)),
                Ok(Args { command: Some(cmd) }) => {
                    let result = self.exec_command(cmd);
                    match result {
                        Err(e) => {
                            match e {
                                Error::Message(message) => eprintln!("Error: {message}"),
                                e => eprintln!("Error: {e:?}"),
                            }
                            Err(())
                        }
                        Ok(()) => Ok(None),
                    }
                }
                Ok(Args { command: None }) => Ok(None),
                Err(clap_err) => {
                    eprintln!("{clap_err}");
                    Err(())
                }
            }
        }
    }
    #[allow(clippy::too_many_lines)] //TODO is this ok?
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
                        self.output(format_args!("added node {node_path}"));
                    }
                    TypedSequencer::FileLines(inner) => {
                        let node_path = if items_filter.is_empty() {
                            inner.add_node(&parent_path, joined)
                        } else {
                            inner.add_terminal_node(&parent_path, joined)
                        }?;
                        self.output(format_args!("added node {node_path}"));
                    }
                    TypedSequencer::FolderListing(inner) => {
                        let node_path = if items_filter.is_empty() {
                            inner.add_node(&parent_path, joined)
                        } else {
                            inner.add_terminal_node(&parent_path, joined)
                        }?;
                        self.output(format_args!("added node {node_path}"));
                    }
                    TypedSequencer::Beet(inner) => {
                        let node_path = if items_filter.is_empty() {
                            inner.add_node(&parent_path, items_filter)
                        } else {
                            inner.add_terminal_node(&parent_path, items_filter)
                        }?;
                        self.output(format_args!("added node {node_path}"));
                    }
                }
            }
            Command::SetFilter { path, items_filter } => {
                let joined = items_filter.join(" ");
                match &mut self.sequencer {
                    TypedSequencer::DebugItem(inner) => {
                        let new = joined;
                        let old = inner.set_node_filter(&path, new.clone())?;
                        self.output(format_args!("changed filter from {old:?} -> {new:?}"));
                    }
                    TypedSequencer::FileLines(inner) => {
                        let new = joined;
                        let old = inner.set_node_filter(&path, new.clone())?;
                        self.output(format_args!("changed filter from {old:?} -> {new:?}"));
                    }
                    TypedSequencer::FolderListing(inner) => {
                        let new = joined;
                        let old = inner.set_node_filter(&path, new.clone())?;
                        self.output(format_args!("changed filter from {old:?} -> {new:?}"));
                    }
                    TypedSequencer::Beet(inner) => {
                        let new = items_filter;
                        let old = inner.set_node_filter(&path, new.clone())?;
                        self.output(format_args!("changed filter from {old:?} -> {new:?}"));
                    }
                }
            }
            Command::SetWeight {
                path,
                item_index,
                weight,
            } => {
                match_seq!(&mut self.sequencer, inner => {
                    let old_weight = if let Some(item_index) = item_index {
                        inner.set_node_item_weight(&path, item_index, weight)?
                    } else {
                        inner.set_node_weight(&path, weight)?
                    };
                    self.output(format_args!("changed weight from {old_weight} -> {weight}"));
                });
            }
            Command::SetOrderType { path, order_type } => {
                match_seq!(&mut self.sequencer, inner => {
                    let old = inner.set_node_order_type(&path, order_type)?;
                    self.output(format_args!("changed order type from {old:?} -> {order_type:?}"));
                });
            }
            Command::Update { path } => {
                let path = path.as_ref().map_or(".", |p| p);
                let path = match_seq!(&mut self.sequencer, inner => inner.update_node(path)?);
                self.output(format_args!("updated nodes under path {path}"));
            }
            Command::Remove { id } => {
                match_seq!(&mut self.sequencer, inner => {
                    let removed = inner.remove_node(&id)?;
                    let (weight, info) = removed;
                    self.output(format_args!("removed node {id}: weight = {weight}, {info:#?}"));
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
    fn output(&self, fmt_args: std::fmt::Arguments) {
        self.params.output(fmt_args);
    }
}

#[derive(Parser)]
struct MainArgs {
    /// Source for the item source
    #[clap(arg_enum)]
    source: Option<ItemSourceType>,
    /// Command to use for the [`Beet`] item source type
    beet_cmd: Option<String>,
    /// Slience non-error output that is not explicitly requested
    #[clap(short, long, action)]
    quiet: bool,
    /// Filename to read commands from, instead of standard-in
    #[clap(long)]
    script: Option<String>,
    /// Terminates on the first error encountered (implied for `--script` mode)
    #[clap(long, action)]
    fatal: bool,
}
#[derive(Clone, ArgEnum)]
enum ItemSourceType {
    Debug,
    FileLines,
    FolderListing,
    Beet,
}
shared::wrapper_enum! {
    enum MainError {
        IO(std::io::Error),
        Message(String),
    }
}
impl std::fmt::Debug for MainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IO(e) => write!(f, "{e}"),
            Self::Message(e) => write!(f, "{e}"),
        }
    }
}
fn main() -> Result<(), MainError> {
    let args = MainArgs::parse();

    if !args.quiet {
        eprint!("{}", COMMAND_NAME);
        eprintln!("{}", shared::license::WELCOME);
    }

    let sequencer = match args.source.unwrap_or(ItemSourceType::FileLines) {
        ItemSourceType::Debug => TypedSequencer::DebugItem(Sequencer::new(DebugItemSource)),
        ItemSourceType::FileLines => {
            TypedSequencer::FileLines(Sequencer::new(FileLines::new(".".into())?))
        }
        ItemSourceType::FolderListing => {
            TypedSequencer::FolderListing(Sequencer::new(FolderListing::new(".".into())?))
        }
        ItemSourceType::Beet => {
            if let Some(beet_cmd) = args.beet_cmd {
                TypedSequencer::Beet(Sequencer::new(Beet::new(beet_cmd)?))
            } else {
                return Err(
                    std::io::Error::new(std::io::ErrorKind::Other, "missing beet_cmd").into(),
                );
            }
        }
    };
    let params = {
        let MainArgs { quiet, fatal, .. } = args;
        let fatal = fatal | args.script.is_some();
        Parameters { quiet, fatal }
    };
    let mut sequencer = Cli::new(sequencer, params);
    if let Some(script) = args.script {
        let script_file = File::open(&script)
            .map_err(|err| format!("unable to open script {script:?}: {err}"))?;
        let script_file = BufReader::new(script_file);
        sequencer.exec_lines(script_file).map_err(|e| {
            match e {
                MainError::Message(msg) => format!("script {script:?} {msg}"),
                MainError::IO(err) => format!("error reading script {script:?}: {err}"),
            }
            .into()
        })
    } else {
        Ok(sequencer.exec_lines(stdin().lock())?)
    }
}
#[cfg(test)]
mod tests {
    use crate::{Args, MainArgs};
    use clap::CommandFactory;

    #[test]
    fn verify_main_args() {
        MainArgs::command().debug_assert();
    }
    #[test]
    fn verify_cli_args() {
        Args::command().debug_assert();
    }
}
