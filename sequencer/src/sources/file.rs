// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use std::{
    fs::File,
    io::{self, BufRead, BufReader},
    path::PathBuf,
};

use super::ItemSource;

pub struct Error {
    path: Option<String>,
    error: std::io::Error,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { path, error } = self;
        if let Some(path) = path {
            write!(f, "path {path:?}: {error}")
        } else {
            write!(f, "path <unknown>: {error}")
        }
    }
}

/// Reads items as lines from the filename specified by the filter args
pub struct Lines {
    root: PathBuf,
}
impl Lines {
    /// Attempts to create an instance with the specified root path
    ///
    /// # Errors
    /// Returns an error if the specified root path is not a directory
    pub fn new(root: PathBuf) -> Result<Self, io::Error> {
        if !root.exists() {
            // TODO change to ErrorKind::NotFound, when stabilized
            return Err(io::Error::new(io::ErrorKind::Other, "not found"));
        }
        if !root.is_dir() {
            // TODO change to ErrorKind::NotADirectory, when stabilized
            return Err(io::Error::new(io::ErrorKind::Other, "not a directory"));
        }
        Ok(Self { root })
    }
}
impl ItemSource<String> for Lines {
    type Item = String;
    type Error = Error;

    fn lookup(&self, args: &[String]) -> Result<Vec<Self::Item>, Self::Error> {
        let mut file_path = self.root.clone();
        for arg in args {
            if !arg.is_empty() {
                file_path.push(arg);
            }
        }
        let err_with_path = |error| {
            let path = file_path.to_str().map(ToOwned::to_owned);
            Error { path, error }
        };
        let file = File::open(&file_path).map_err(err_with_path)?;
        BufReader::new(file)
            .lines()
            .filter(|r| match r {
                Ok(line) => !line.is_empty(),
                Err(..) => true,
            })
            .collect::<Result<_, _>>()
            .map_err(err_with_path)
    }
}
