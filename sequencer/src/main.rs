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
use q_filter_tree::{id::NodePathTyped, OrderType, Weight};
use std::{
    fs::File,
    io::{stdin, BufRead, BufReader},
};

use arg_split::ArgSplit;
use sequencer::{sources::multi_select::Mismatch, Error, Sequencer};

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
        let sequencer = Sequencer::new(source);
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
                        self.sequencer.add_terminal_node(&parent_path, Some(filter))
                    } else {
                        self.sequencer.add_node(&parent_path, None)
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
                let old = self.sequencer.set_node_filter(&path, filter.clone())?;
                self.output(format_args!("changed filter from {old:?} -> {filter:?}"));
            }
            Command::SetWeight {
                path,
                item_index,
                weight,
            } => {
                let old_weight = if let Some(item_index) = item_index {
                    self.sequencer
                        .set_node_item_weight(&path, item_index, weight)?
                } else {
                    self.sequencer.set_node_weight(&path, weight)?
                };
                self.output(format_args!("changed weight from {old_weight} -> {weight}"));
            }
            Command::SetOrderType { path, order_type } => {
                let old = self.sequencer.set_node_order_type(&path, order_type)?;
                self.output(format_args!(
                    "changed order type from {old:?} -> {order_type:?}"
                ));
            }
            Command::Update { path } => {
                let path = path.as_ref().map_or(".", |p| p);
                let path = self.sequencer.update_node(path)?;
                self.output(format_args!("updated nodes under path {path}"));
            }
            Command::Remove { id } => {
                let removed = self.sequencer.remove_node(&id)?;
                let (weight, info) = removed;
                self.output(format_args!(
                    "removed node {id}: weight = {weight}, {info:#?}"
                ));
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
        }
        Ok(())
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
        let mut existing_path_type = Ok(None);
        let mut accumulator = |path: &NodePathTyped, filter: &Option<source::TypedArg>| {
            let new_type = filter.as_ref().map(source::Type::from);
            // detect and **REPORT** bad state
            if let Ok(existing_opt) = &mut existing_path_type {
                //TODO simplify in the future using Option::unzip
                // [tracking issue for Option::unzip](https://github.com/rust-lang/rust/issues/87800)
                let (existing_path, existing_type) = if let Some((path, ty)) = existing_opt.take() {
                    (Some(path), Some(ty))
                } else {
                    (None, None)
                };
                existing_path_type = Mismatch::combine_verify(new_type, existing_type)
                    .map(|matched| matched.map(|ty| (path.clone(), ty)))
                    .map_err(|mismatch| {
                        let existing_path_str = existing_path
                            .map_or_else(String::default, |p| format!(" from path {p}"));
                        format!("at path {path} arg type {mismatch} from path {existing_path_str}")
                    });
            }
        };
        self.sequencer
            .with_ancestor_filters(path, &mut accumulator)?;
        let existing_type = existing_path_type?.map(|(_, ty)| ty);
        let source_type =
            Mismatch::combine_verify(existing_type, requested_type).map_err(|e| format!("{e}"))?;
        Ok(source_type)
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
