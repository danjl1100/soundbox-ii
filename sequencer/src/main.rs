// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use clap::Parser;
use std::{
    io::{stdin, BufRead},
    str::FromStr,
};

use arg_split::ArgSplit;
use q_filter_tree::id::{NodePath, NodePathTyped};
use sequencer::Sequencer;

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

#[derive(Parser, Debug)]
enum Command {
    Show,
    AddNode {
        parent_path: Option<String>,
    },
    SetNodeFile {
        node_path: Option<String>,
        filename: String,
    },
}

#[derive(Default)]
struct Cli {
    sequencer: Sequencer,
}
impl Cli {
    fn exec_lines<T>(&mut self, input: T) -> Result<(), std::io::Error>
    where
        T: BufRead,
    {
        for line in input.lines() {
            let line = line?;
            match Args::try_from(&line) {
                Ok(Args { command: Some(cmd) }) => {
                    let result = self.exec_command(cmd);
                    if let Err(e) = result {
                        eprintln!("ERROR: {:?}", e);
                    }
                }
                Ok(Args { command: None }) => continue,
                Err(clap_err) => {
                    eprintln!("unrecognized command: {}", clap_err);
                    continue;
                }
            }
        }
        Ok(())
    }
    fn exec_command(&mut self, command: Command) -> Result<(), CommandError> {
        match command {
            Command::Show => {
                println!("{}", self.sequencer);
            }
            Command::AddNode { parent_path } => {
                let node_path = self
                    .sequencer
                    .add_node(parent_path.unwrap_or_default())
                    .map_err(CommandError::from)?;
                println!("added node {node_path}");
            }
            Command::SetNodeFile {
                node_path,
                filename,
            } => {
                self.sequencer
                    .set_node_file(node_path.unwrap_or_default(), filename)?;
            }
        }
        Ok(())
    }
}

shared::wrapper_enum! {
    #[derive(Debug)]
    enum CommandError {
        Serde(serde_json::Error),
        Sequencer(sequencer::Error),
    }
}

fn main() -> Result<(), std::io::Error> {
    println!("hello, sequencer!");

    Cli::default().exec_lines(stdin().lock())
}
