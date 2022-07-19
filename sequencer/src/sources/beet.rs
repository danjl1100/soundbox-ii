// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use super::ItemSource;
use std::{
    ffi::OsStr,
    io::{BufRead, BufReader, Error, ErrorKind},
    ops::Not,
    path::PathBuf,
    process::{Command, Stdio},
};

pub trait ArgSource {
    type Arg: AsRef<OsStr>;
    fn get_beet_args(&self) -> &[Self::Arg];
}
impl<T> ArgSource for Vec<T>
where
    T: AsRef<OsStr>,
{
    type Arg = T;
    fn get_beet_args(&self) -> &[T] {
        self
    }
}

/// Queries the [beets] database per the supplied filter arguments
///
/// [beets]: (https://beets.io/)
pub struct Beet {
    command: PathBuf,
}
impl Beet {
    /// Attempts to create a new Beet item source
    ///
    /// # Errors
    /// Returns an error if the specified command does not exist
    pub fn new(command: String) -> Result<Self, std::io::Error> {
        let command = PathBuf::from(command);
        if command.is_file() {
            Ok(Self { command })
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "file not found",
            ))
        }
    }
}
impl<T: ArgSource> ItemSource<T> for Beet {
    type Item = String;
    type Error = std::io::Error;

    fn lookup(&self, args: &[T]) -> Result<Vec<Self::Item>, Self::Error> {
        let arg_elems: Vec<_> = args.iter().flat_map(ArgSource::get_beet_args).collect();
        let mut child = Command::new(&self.command)
            .args(arg_elems)
            .stdout(Stdio::piped())
            .spawn()?;
        let stdout = child
            .stdout
            .as_mut()
            .ok_or_else(|| Error::new(ErrorKind::Other, "unable to capture stdout"))?;
        let reader = BufReader::new(stdout);
        let items = reader
            .lines()
            .filter_map(|line| {
                line.map(|line| {
                    let line = line.trim();
                    line.is_empty().not().then_some(line).map(str::to_string)
                })
                .transpose()
            })
            .collect();
        let result = child.wait()?;
        if result.success() {
            items
        } else {
            //TODO consider capturing stderr to provide more info in an error message
            //  (how to condense stderr down to a plain error message? last line?)
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("child process failure: {result}"),
            ))
        }
    }
}
