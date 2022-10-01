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
    command::{self, Runnable},
    Error, Sequencer,
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
pub(crate) enum Command {
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
        /// Type of the source (defaults to main-args option)
        #[clap(long, arg_enum)]
        source_type: Option<source::Type>,
    },
    /// Set the filter for the specified node
    SetFilter {
        /// Path of the node to modify
        path: String,
        /// New filter value
        items_filter: Vec<String>,
        /// Type of the source (defaults to main-args option)
        #[clap(long, arg_enum)]
        source_type: Option<source::Type>,
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
        //TODO is this appropriate?
        // recursive: bool,
    },
    /// Print the next item(s)
    #[clap(alias("n"))]
    Next {
        /// Number of items to print
        count: Option<usize>,
    },
    /// Set the minimum number of staged (determined) items at the root node
    #[clap(alias("prefill"))]
    SetPrefill {
        /// Minimum number of items to stage
        min_count: usize,
        /// Path of the target node (default is root)
        path: Option<String>,
    },
    /// Removes an item from the queue of the specified node
    QueueRemove {
        /// Index of the queue item to remove
        index: usize,
        /// Path of the target node (default is root)
        path: Option<String>,
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
    sequencer: Sequencer<source::Source, Option<source::TypedArg>>,
    source_type: source::Type,
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
mod source {
    use clap::ValueEnum;
    use serde::Serialize;
    use std::path::PathBuf;

    use sequencer::{
        sources::{Beet, FileLines, FolderListing},
        DebugItemSource,
    };
    sequencer::source_multi_select! {
        pub(crate) struct Source {
            type Args = Args<'a>;
            #[derive(Copy, Clone, ValueEnum)]
            type Type = Type;
            /// Beet
            beet: Beet as Beet where arg type = Vec<String>,
            /// File lines
            file_lines: FileLines as FileLines where arg type = String,
            /// Folder listing
            folder_listing: FolderListing as FolderListing where arg type = String,
            /// Debug
            debug: DebugItemSource as Debug where arg type = String,
        }
        #[derive(Clone, Debug, Serialize)]
        /// Typed argument
        impl ItemSource<Option<TypedArg>> {
            type Item = String;
            /// Typed Error
            type Error = TypedLookupError;
        }
    }
    impl Source {
        pub fn new(root_folder: PathBuf, beet_cmd: String) -> Result<Self, super::MainError> {
            Ok(Self {
                debug: DebugItemSource,
                file_lines: FileLines::new(root_folder.clone())?,
                folder_listing: FolderListing::new(root_folder)?,
                beet: Beet::new(beet_cmd)?,
            })
        }
    }
}

impl Cli {
    const COMMENT: &'static str = "#";
    fn new(source: source::Source, source_type: source::Type, params: Parameters) -> Self {
        let sequencer: Sequencer<source::Source, Option<source::TypedArg>> =
            Sequencer::new(source, None);
        Self {
            sequencer,
            source_type,
            params,
        }
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
                let sequencer = &self.sequencer;
                println!("{sequencer}");
            }
            Command::Add {
                parent_path,
                items_filter,
                source_type: requested_type,
            } => {
                let source_type = self.calculate_existing_type(&parent_path, requested_type)?;
                let node_path =
                    if let Some(filter) = self.parse_filter_args(items_filter, source_type) {
                        self.run(command::AddTerminalNode {
                            parent_path,
                            filter: Some(filter),
                        })
                    } else {
                        self.run(command::AddNode {
                            parent_path,
                            filter: None,
                        })
                    }?;
                self.output(format_args!("added node {node_path}"));
            }
            Command::SetFilter {
                path,
                items_filter,
                source_type: requested_type,
            } => {
                let source_type = self.calculate_existing_type(&path, requested_type)?;
                let filter = self.parse_filter_args(items_filter, source_type);
                let filter_print = filter.clone();
                let old = self.run(command::SetNodeFilter { path, filter });
                self.output(format_args!(
                    "changed filter from {old:?} -> {filter_print:?}"
                ));
            }
            Command::SetWeight {
                path,
                item_index,
                weight,
            } => {
                let old_weight = if let Some(item_index) = item_index {
                    self.run(command::SetNodeItemWeight {
                        path,
                        item_index,
                        weight,
                    })
                } else {
                    self.run(command::SetNodeWeight { path, weight })
                }?;
                self.output(format_args!("changed weight from {old_weight} -> {weight}"));
            }
            Command::SetOrderType { path, order_type } => {
                let old = self.run(command::SetNodeOrderType { path, order_type })?;
                self.output(format_args!(
                    "changed order type from {old:?} -> {order_type:?}"
                ));
            }
            Command::Update { path } => {
                let path = path.unwrap_or_else(|| ".".to_string());
                let path_print = path.clone();
                self.run(command::UpdateNodes { path })?;
                self.output(format_args!("updated nodes under path {path_print}"));
            }
            Command::Remove { id } => {
                let id_print = id.clone();
                self.run(command::RemoveNode { id })?;
                // let removed = self.sequencer.remove_node(&id)?;
                // let (weight, info) = removed;
                self.output(format_args!("removed node {id_print}"));
            }
            Command::Next { count } => {
                let count = count.unwrap_or(1);
                for _ in 0..count {
                    let popped = self.sequencer.pop_next();
                    if let Some(item) = popped {
                        println!("Item {item:?}");
                    } else {
                        println!("No items remaining");
                        break;
                    }
                }
            }
            Command::SetPrefill { path, min_count } => {
                self.run(command::SetNodePrefill { path, min_count })?;
            }
            Command::QueueRemove { index, path } => {
                self.run(command::QueueRemove { path, index })?;
            }
        }
        Ok(())
    }
    fn run<T>(&mut self, command: T) -> Result<T::Output, Error>
    where
        T: Runnable<Option<source::TypedArg>>,
    {
        self.sequencer.run(command)
    }
    fn parse_filter_args(
        &self,
        items_filter: Vec<String>,
        source_type: Option<source::Type>,
    ) -> Option<source::TypedArg> {
        if items_filter.is_empty() {
            None
        } else {
            let joined = items_filter.join(" ");
            let filter = match source_type.unwrap_or(self.source_type) {
                source::Type::Debug => source::TypedArg::Debug(joined),
                source::Type::FileLines => source::TypedArg::FileLines(joined),
                source::Type::FolderListing => source::TypedArg::FolderListing(joined),
                source::Type::Beet => source::TypedArg::Beet(items_filter),
            };
            Some(filter)
        }
    }
    fn calculate_existing_type(
        &self,
        path: &str,
        requested_type: Option<source::Type>,
    ) -> Result<Option<source::Type>, Error> {
        self.sequencer
            .calculate_required_type(path, requested_type)?
            .map_err(|mismatch_label| format!("{mismatch_label}").into())
    }
    fn output(&self, fmt_args: std::fmt::Arguments) {
        self.params.output(fmt_args);
    }
}

#[derive(Parser)]
struct MainArgs {
    /// Command to use for the [`Beet`] item source type
    #[clap(long)]
    beet_cmd: String,
    /// Initial default source type used for setting filters
    #[clap(long, arg_enum)]
    source_type: Option<source::Type>,
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
        PathIO(sequencer::sources::PathError),
    }
}
impl std::fmt::Debug for MainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self}")
    }
}
impl std::fmt::Display for MainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IO(e) => write!(f, "{e}"),
            Self::Message(e) => write!(f, "{e}"),
            Self::PathIO(e) => write!(f, "{e}"),
        }
    }
}
fn main() -> Result<(), MainError> {
    let args = MainArgs::parse();

    if !args.quiet {
        eprint!("{}", COMMAND_NAME);
        eprintln!("{}", shared::license::WELCOME);
    }

    let source_type = args.source_type.unwrap_or(source::Type::FileLines);
    let root_path = ".".into();
    let beet_cmd = args.beet_cmd; // .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::Other, "missing beet_cmd"))?;
    let source = source::Source::new(root_path, beet_cmd)?;
    let params = {
        let MainArgs { quiet, fatal, .. } = args;
        let fatal = fatal | args.script.is_some();
        Parameters { quiet, fatal }
    };
    let mut sequencer = Cli::new(source, source_type, params);
    if let Some(script) = args.script {
        let script_file = File::open(&script)
            .map_err(|err| format!("unable to open script {script:?}: {err}"))?;
        let script_file = BufReader::new(script_file);
        sequencer
            .exec_lines(script_file)
            .map_err(|e| format!("script {script:?} {e}").into())
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
