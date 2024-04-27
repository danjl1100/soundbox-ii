// Copyright (C) 2021-2023  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
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

use arg_util::ArgSplit;
use clap::{Parser, ValueEnum};
use sequencer::{
    cli::{NodeCommand, OutputParams},
    persistence::SequencerConfigFile,
};
use std::{
    fs::File,
    io::{stdin, BufRead, BufReader},
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
        let line_parts = ArgSplit::split_into_owned(line);
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
    /// Print the next item(s)
    #[clap(alias("n"))]
    Next {
        /// Number of items to print
        count: Option<usize>,
    },
    #[clap(subcommand)]
    Node(NodeCommand<source::Type>),
    #[clap(subcommand)]
    Persist(PersistenceCommand),
}
/// Types of License snippets available to show
#[derive(clap::Subcommand, Clone, Copy, Debug)]
pub enum ShowCopyingLicenseType {
    /// Show warranty details
    #[clap(alias("w"))]
    Warranty,
    /// Show conditions for redistribution
    #[clap(alias("c"))]
    Copying,
}

#[derive(Parser, Debug)]
enum PersistenceCommand {
    Load {
        file: std::path::PathBuf,
    },
    Save {
        file: std::path::PathBuf,
        #[clap(long)]
        overwrite: bool,
    },
}

struct Cli {
    #[allow(clippy::struct_field_names)]
    sequencer_cli: sequencer::cli::Cli<source::Source, source::FilterArgParser, source::TypedArg>,
    sequencer_config_file: Option<SequencerConfigFile<String, Option<source::TypedArg>>>,
    /// Terminates on the first error encountered (implied for `--script` mode)
    fatal: bool,
    /// If true, echo all commands to stdout
    echo_commands: bool,
}

mod source {
    use clap::ValueEnum;
    use serde::{Deserialize, Serialize};
    use std::path::PathBuf;

    use sequencer::{
        persistence::OptionStructSerializeDeserialize,
        sources::{Beet, FileLines, FolderListing},
        DebugItemSource,
    };
    sequencer::source_multi_select! {
        #[derive(Clone)]
        pub(crate) struct Source {
            type Args = Args<'a>;
            #[derive(Copy, Clone, ValueEnum)]
            type Type = Type;
            /// Beet
            beet: Option<Beet> as Beet where arg type = Vec<String>,
            /// File lines
            file_lines: FileLines as FileLines where arg type = String,
            /// Folder listing
            folder_listing: FolderListing as FolderListing where arg type = String,
            /// Debug
            debug: DebugItemSource as Debug where arg type = String,
        }
        #[derive(Clone, Debug, Serialize, Deserialize)]
        /// Typed argument
        impl ItemSource<Option<TypedArg>> {
            type Item = String;
            /// Typed Error
            type Error = TypedLookupError;
        }
    }
    impl OptionStructSerializeDeserialize for TypedArg {}
    impl Source {
        pub fn new(
            root_folder: PathBuf,
            beet_cmd: Option<String>,
        ) -> Result<Self, super::MainError> {
            let beet = beet_cmd.map(Beet::new).transpose()?;
            Ok(Self {
                debug: DebugItemSource,
                file_lines: FileLines::new(root_folder.clone())?,
                folder_listing: FolderListing::new(root_folder)?,
                beet,
            })
        }
    }
    pub(super) struct FilterArgParser {
        pub default_type: Type,
    }
    impl sequencer::cli::FilterArgParser for FilterArgParser {
        type Type = Type;
        type Filter = TypedArg;
        fn parse_filter_args(
            &self,
            args: Vec<String>,
            source_type: Option<Type>,
        ) -> Option<TypedArg> {
            if args.is_empty() {
                None
            } else {
                let joined = args.join(" ");
                let filter = match source_type.unwrap_or(self.default_type) {
                    Type::Debug => TypedArg::Debug(joined),
                    Type::FileLines => TypedArg::FileLines(joined),
                    Type::FolderListing => TypedArg::FolderListing(joined),
                    Type::Beet => TypedArg::Beet(args),
                };
                Some(filter)
            }
        }
    }
}
impl Cli {
    fn exec_lines<V>(&mut self, input: V) -> Result<(), MainError>
    where
        V: BufRead,
    {
        for (line_number, line) in input.lines().enumerate() {
            let line = line?;
            match self.exec_line(&line) {
                Ok(Some(shared::Shutdown)) => {
                    self.sequencer_cli.output(format_args!("exited cleanly"));
                    return Ok(());
                }
                Err(()) if self.fatal => {
                    let line_number = line_number + 1; // human-readable one-based counting
                    Err(format!("failed on line {line_number}: {line:?}"))?;
                }
                Ok(None) | Err(()) => {}
            }
        }
        self.sequencer_cli.output(format_args!("<<EOF>>"));
        Ok(())
    }
    const COMMENT: &'static str = "#";
    fn exec_line(&mut self, line: &str) -> Result<Option<shared::Shutdown>, ()> {
        if line.trim_start().starts_with(Self::COMMENT) {
            Ok(None)
        } else {
            match Args::try_from(line) {
                Ok(Args { command: Some(cmd) }) => {
                    self.print_echo_command(line);
                    match cmd {
                        Command::Quit => Ok(Some(shared::Shutdown)),
                        Command::Show { license } => {
                            Self::show_license(license);
                            Ok(None)
                        }
                        Command::Next { count } => {
                            let count = count.unwrap_or(1);
                            self.print_next(count);
                            Ok(None)
                        }
                        Command::Node(node_command) => {
                            let result = self.sequencer_cli.exec_command(node_command);
                            if let Err(err) = &result {
                                eprintln!("Node command error: {err}");
                            }
                            result.map(|()| None).map_err(|_| ())
                        }
                        Command::Persist(persist_command) => {
                            let result = self.exec_persist_command(persist_command);
                            if let Err(err) = &result {
                                eprintln!("Persist command error: {err}");
                            }
                            result.map(|()| None).map_err(|_| ())
                        }
                    }
                }
                Ok(Args { command: None }) => Ok(None),
                Err(clap_err) => {
                    self.print_echo_command(line);
                    eprintln!("{clap_err}");
                    Err(())
                }
            }
        }
    }
    fn print_echo_command(&self, line: &str) {
        if self.echo_commands {
            println!("> {line}");
        }
    }
    fn print_next(&mut self, count: usize) {
        let mut count_actual = 0;
        for _ in 0..count {
            let popped = self.sequencer_cli.pop_next();
            if let Some(item) = popped {
                let (node_seq, item) = item.into_parts();
                println!("Item {item:?}, from node #{node_seq}");
            } else {
                println!("No items remaining");
                break;
            }
            count_actual += 1;
        }
        self.sequencer_cli
            .output(format_args!("printed {count_actual} items"));
    }
    fn show_license(license: ShowCopyingLicenseType) {
        match license {
            ShowCopyingLicenseType::Warranty => {
                eprintln!("{}", shared::license::WARRANTY);
            }
            ShowCopyingLicenseType::Copying => {
                eprintln!("{}", shared::license::REDISTRIBUTION);
            }
        }
    }
    fn exec_persist_command(&mut self, cmd: PersistenceCommand) -> Result<(), MainError> {
        match cmd {
            PersistenceCommand::Load { file } => {
                let result: Result<(_, sequencer::SequencerTree<String, _>), _> =
                    SequencerConfigFile::read_from_file(file);
                match result {
                    Ok((scf, tree)) => {
                        self.sequencer_config_file = Some(scf);
                        self.sequencer_cli.replace_sequencer_tree(tree);
                        Ok(())
                    }
                    Err(err) => Err(MainError::Message(format!("{err}"))),
                }
            }
            PersistenceCommand::Save { file, overwrite } => {
                let sequencer_tree: &sequencer::SequencerTree<_, _> =
                    self.sequencer_cli.sequencer().sequencer_tree();
                match &mut self.sequencer_config_file {
                    Some(scf) if *scf.path() == file => {
                        let result = scf.update_to_file(sequencer_tree);
                        result.map_err(|err| MainError::Message(format!("{err}")))
                    }
                    _ => {
                        let result =
                            SequencerConfigFile::create_new_file(file, sequencer_tree, overwrite);
                        match result {
                            Ok(scf) => {
                                self.sequencer_config_file = Some(scf);
                                Ok(())
                            }
                            Err(err) => Err(MainError::Message(format!("{err}"))),
                        }
                    }
                }
            }
        }
    }
}

#[derive(Parser)]
struct MainArgs {
    /// Command to use for the [`Beet`] item source type
    #[clap(long)]
    beet_cmd: Option<String>,
    /// Initial default source type used for setting filters
    #[clap(long, value_enum)]
    source_type: Option<source::Type>,
    /// Slience non-error output that is not explicitly requested
    #[clap(short, long, action)]
    quiet: bool,
    /// Echo all commands to stdout
    #[clap(long, action)]
    echo_commands: bool,
    /// Filename to read commands from, instead of standard-in
    #[clap(long)]
    script: Option<String>,
    /// Terminates on the first error encountered (implied for `--script` mode)
    #[clap(long, action)]
    fatal: bool,
}
#[derive(Clone, ValueEnum)]
enum ItemSourceType {
    Debug,
    FileLines,
    FolderListing,
    Beet,
}
impl From<&MainArgs> for OutputParams {
    fn from(args: &MainArgs) -> Self {
        Self { quiet: args.quiet }
    }
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
        use shared::license::WELCOME;
        eprint!("{COMMAND_NAME}");
        eprintln!("{WELCOME}");
    }

    let source_type = args.source_type.unwrap_or(source::Type::FileLines);
    let root_path = ".".into();
    let params = OutputParams::from(&args);
    let beet_cmd = args.beet_cmd; // .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::Other, "missing beet_cmd"))?;
    let source = source::Source::new(root_path, beet_cmd)?;
    let filter_arg_parser = source::FilterArgParser {
        default_type: source_type,
    };
    let fatal = args.fatal | args.script.is_some();
    let preloaded_tree = None;
    let mut cli = Cli {
        sequencer_cli: sequencer::cli::Cli::new(source, filter_arg_parser, params, preloaded_tree),
        sequencer_config_file: None,
        fatal,
        echo_commands: args.echo_commands,
    };
    if let Some(script) = args.script {
        let script_file = File::open(&script)
            .map_err(|err| format!("unable to open script {script:?}: {err}"))?;
        let script_file = BufReader::new(script_file);
        cli.exec_lines(script_file)
            .map_err(|e| format!("script {script:?} {e}").into())
    } else {
        Ok(cli.exec_lines(stdin().lock())?)
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
