// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! [`ItemSource`] types

use std::{ffi::OsStr, io, path::PathBuf};

pub use file::FolderListing;
pub use file::Lines as FileLines;
mod file;

pub use beet::Beet;
mod beet;

/// Source of items for the [`Sequencer`](`super::Sequencer`)
///
/// Generic `T` is the argument to the lookup, from each node in path to the terminal items node
pub trait ItemSource<T> {
    /// Element resulting from the lookup
    type Item: serde::Serialize + Clone + PartialEq;
    /// Error type if the lookup fails
    type Error: std::fmt::Display;
    /// Retrieves [`Item`](`Self::Item`)s matching the specified arguments (`T`)
    ///
    /// # Errors
    /// Returns an error if the underlying lookup operation fails
    fn lookup(&self, args: &[T]) -> Result<Vec<Self::Item>, Self::Error>;
}

struct RootFolder(PathBuf);
impl RootFolder {
    /// Attempts to reference the specified root path
    ///
    /// # Errors
    /// Returns an error if the specified root path is not a directory
    fn new(root: PathBuf) -> Result<Self, io::Error> {
        if !root.exists() {
            // TODO change to ErrorKind::NotFound, when stabilized
            return Err(io::Error::new(io::ErrorKind::Other, "not found"));
        }
        if !root.is_dir() {
            // TODO change to ErrorKind::NotADirectory, when stabilized
            return Err(io::Error::new(io::ErrorKind::Other, "not a directory"));
        }
        Ok(Self(root))
    }
    fn clone_to_child_path<T>(&self, path_elems: &[T]) -> PathBuf
    where
        T: AsRef<OsStr>,
    {
        let mut child_path = self.as_ref().clone();
        for path_elem in path_elems.iter().map(AsRef::as_ref) {
            if !path_elem.is_empty() {
                child_path.push(path_elem);
            }
        }
        child_path
    }
}
impl AsRef<PathBuf> for RootFolder {
    fn as_ref(&self) -> &PathBuf {
        &self.0
    }
}

/// [`std::io::Error`] with an associated path context
pub struct PathError {
    /// Textual representation of the path (if the path is valid UTF-8)
    path: Option<String>,
    error: std::io::Error,
}
impl std::fmt::Display for PathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { path, error } = self;
        if let Some(path) = path {
            write!(f, "path {path:?}: {error}")
        } else {
            write!(f, "path <unknown>: {error}")
        }
    }
}
impl PathError {
    fn with_path_fn(file_path: PathBuf) -> impl Fn(std::io::Error) -> Self {
        move |error| {
            let path = file_path.to_str().map(ToOwned::to_owned);
            PathError { path, error }
        }
    }
}
