// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use std::io::{stdin, BufRead};

use arg_split::ArgSplit;
use q_filter_tree::Tree;

use clap::Parser;
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
}

struct Sequencer {
    tree: Tree<String, ()>,
}
impl Sequencer {
    fn new() -> Self {
        Self { tree: Tree::new() }
    }
    fn exec_lines<T>(&mut self, input: T) -> Result<(), std::io::Error>
    where
        T: BufRead,
    {
        for line in input.lines() {
            let line = line?;
            match Args::try_from(&line) {
                Ok(Args { command: Some(cmd) }) => self.exec_command(cmd),
                Ok(Args { command: None }) => continue,
                Err(clap_err) => {
                    eprintln!("unrecognized command: {}", clap_err);
                    continue;
                }
            }
        }
        Ok(())
    }
    fn exec_command(&mut self, command: Command) {
        println!("command: {:?}", command);
        match command {
            Command::Show => {
                println!("{:?}", self.tree);
            }
        }
    }
}

fn main() -> Result<(), std::io::Error> {
    println!("hello, sequencer!");

    Sequencer::new().exec_lines(stdin().lock())
}
