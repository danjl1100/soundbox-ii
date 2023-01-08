// Copyright (C) 2021-2023  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use std::{
    ffi::OsStr,
    fs::File,
    io::{self, BufRead, BufReader},
    path::PathBuf,
};
use walkdir::WalkDir;

use super::{ItemSource, PathError, RootFolder};

/// Reads items as lines from the filename specified by the filter args
#[derive(Clone)]
pub struct Lines {
    root: RootFolder,
}
impl Lines {
    /// Attempts to create an instance with the specified root path
    ///
    /// # Errors
    /// Returns an error if the specified root path is not a directory
    pub fn new(root: PathBuf) -> Result<Self, io::Error> {
        Ok(Self::from(RootFolder::new(root)?))
    }
}
impl From<RootFolder> for Lines {
    fn from(root: RootFolder) -> Self {
        Self { root }
    }
}
impl<T> ItemSource<T> for Lines
where
    T: AsRef<OsStr>,
{
    type Item = String;
    type Error = PathError;

    fn lookup(&self, args: &[T]) -> Result<Vec<Self::Item>, Self::Error> {
        let file_path = self.root.clone_to_child_path(args);
        let file = File::open(&file_path);
        let err_with_path = PathError::with_path_fn(file_path);
        let file = file.map_err(&err_with_path)?;
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

/// Lists files recursively from the folder specified by the filter args
#[derive(Clone)]
pub struct FolderListing {
    root: RootFolder,
}
impl FolderListing {
    /// Attempts to create an instance with the specified root path
    ///
    /// # Errors
    /// Returns an error if the specified root path is not a directory
    pub fn new(root: PathBuf) -> Result<Self, io::Error> {
        Ok(Self::from(RootFolder::new(root)?))
    }
}
impl From<RootFolder> for FolderListing {
    fn from(root: RootFolder) -> Self {
        Self { root }
    }
}
impl<T> ItemSource<T> for FolderListing
where
    T: AsRef<OsStr>,
{
    type Item = String;
    type Error = PathError;

    fn lookup(&self, args: &[T]) -> Result<Vec<Self::Item>, Self::Error> {
        let folder_path = self.root.clone_to_child_path(args);
        let mut files: Vec<_> = WalkDir::new(folder_path)
            .into_iter()
            // ignore errors (usually permission errors)
            .filter_map(Result::ok)
            // ignore folders
            .filter(|entry| !entry.path().is_dir())
            // clone into String, ignore non-UTF8 filenames
            .filter_map(|entry| entry.path().to_str().map(String::from))
            .collect();
        // ensure determinstic ordering
        files.sort();
        Ok(files)
    }
}
