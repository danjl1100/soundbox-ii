// Copyright (C) 2021-2023  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use super::{ItemSource, PathError};
use std::{
    ffi::{OsStr, OsString},
    io::{BufRead, BufReader},
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
impl<T> ArgSource for &Vec<T>
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
#[derive(Clone)]
pub struct Beet {
    command: PathBuf,
}
impl Beet {
    /// Attempts to create a new Beet item source
    ///
    /// # Errors
    /// Returns an error if the specified command does not exist
    pub fn new(command: String) -> Result<Self, PathError> {
        let command = PathBuf::from(command);
        // TODO add a timeout, or print-out a message in case of infinite-hanging process provided
        let canary_result = Command::new(&command)
            .args(BEET_ARGS_VERSION)
            .stdout(Stdio::null())
            .status();
        let err = match canary_result {
            Ok(status) if status.success() => {
                return Ok(Self { command });
            }
            Ok(status) => std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("beet command executable returned non-zero result {status}"),
            ),
            Err(e) => e,
        };
        Err(PathError::new(&command, err))
    }
    /// Returns a displayable representation of the command
    pub fn display(&self) -> impl std::fmt::Display + '_ {
        self.command.display()
    }
}
const BEET_ARGS_VERSION: &[&str] = &["--version"];
const BEET_ARGS_LOOKUP: &[&str] = &["ls", "-p"];
impl<T: ArgSource> ItemSource<T> for Beet {
    type Item = String;
    type Error = std::io::Error;

    fn lookup(&self, args: &[T]) -> Result<Vec<Self::Item>, Self::Error> {
        let arg_elems: Vec<_> = BEET_ARGS_LOOKUP
            .iter()
            .map(OsString::from)
            .chain(
                args.iter()
                    .flat_map(ArgSource::get_beet_args)
                    .map(OsString::from),
            )
            .collect();
        let mut child = Command::new(&self.command)
            .args(arg_elems)
            .stdout(Stdio::piped())
            .spawn()?;
        let stdout = child.stdout.as_mut().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::Other, "unable to capture stdout")
        })?;
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

pub enum ErrorOptionalBeet {
    IO(std::io::Error),
    None,
}
impl std::fmt::Display for ErrorOptionalBeet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorOptionalBeet::IO(inner) => write!(f, "{inner}"),
            ErrorOptionalBeet::None => write!(f, "optional beet disabled at runtime"),
        }
    }
}

impl<T: ArgSource> ItemSource<T> for Option<Beet> {
    type Item = String;
    type Error = ErrorOptionalBeet;

    fn lookup(&self, args: &[T]) -> Result<Vec<Self::Item>, Self::Error> {
        if let Some(inner) = self {
            inner.lookup(args).map_err(ErrorOptionalBeet::IO)
        } else {
            Err(ErrorOptionalBeet::None)
        }
    }
}
