// Copyright (C) 2021-2023  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Contains the IO wrapper [`SequencerConfigFile`] around [`SequencerConfig`]

use super::{FromKdlEntries, IntoKdlEntries, ParseError, SequencerConfig, SequencerTree};
use shared::Never;

/// Wrapper around [`SequencerConfig`] with a file open for subsequent writing
pub struct SequencerConfigFile<T, F> {
    inner: SequencerConfig<T, F>,
    path: std::path::PathBuf,
    file: std::fs::File,
}
impl<T, F> SequencerConfigFile<T, F> {
    /// Returns the path to be used for subsequent updates
    pub fn path(&self) -> &std::path::PathBuf {
        &self.path
    }
}
impl<T, F> SequencerConfigFile<T, F>
where
    T: Clone,
    F: FromKdlEntries + IntoKdlEntries,
{
    /// Creates a new file, immediately updating for the specified [`SequencerTree`]
    pub fn create_new_file(
        path: std::path::PathBuf,
        sequencer: &SequencerTree<T, F>,
        allow_overwrite: bool,
    ) -> Result<Self, WriteError<F>> {
        let mut file = std::fs::OpenOptions::new();
        if allow_overwrite {
            file.create(true);
        } else {
            file.create_new(true);
        }
        let file = file.write(true).open(&path).map_err(|err| WriteError {
            path: path.clone(),
            kind: WriteErrorKind::IO(err),
        })?;
        let mut this = Self {
            inner: SequencerConfig::default(),
            path,
            file,
        };
        this.update_to_file(sequencer).map(|()| this)
    }

    /// Reads the config from a KDL file
    ///
    /// # Errors
    /// Returns an error if the IO fails or the file contents is invalid
    pub fn read_from_file(
        path: impl AsRef<std::path::Path>,
    ) -> Result<(Self, SequencerTree<T, F>), ReadError<F>> {
        let path = path.as_ref();
        Self::read_from_file_inner(path).map_err(|kind| ReadError {
            path: path.to_owned(),
            kind,
        })
    }
    fn read_from_file_inner(
        path: &std::path::Path,
    ) -> Result<(Self, SequencerTree<T, F>), ReadErrorKind<F>> {
        use std::io::Read;

        let mut file = std::fs::File::open(path).map_err(ReadErrorKind::IO)?;
        let contents = {
            let mut contents = String::new();
            file.read_to_string(&mut contents)
                .map_err(ReadErrorKind::IO)?;
            contents
        };

        SequencerConfig::parse_from_str(&contents)
            .map(|(inner, sequencer)| {
                (
                    Self {
                        inner,
                        path: path.to_owned(),
                        file,
                    },
                    sequencer,
                )
            })
            .map_err(ReadErrorKind::Parse)
    }

    /// Updates the internal KDL document to match the specified [`SequencerTree`] and writes the
    /// KDL text to the file
    ///
    /// # Errors
    /// Returns an error if IO fails
    pub fn update_to_file(&mut self, sequencer: &SequencerTree<T, F>) -> Result<(), WriteError<F>> {
        use std::io::Write;

        let contents = self
            .inner
            .update_to_string(sequencer)
            .map_err(|err| WriteError {
                path: self.path.clone(),
                kind: WriteErrorKind::Serialize(err),
            })?;
        self.file
            .write_all(contents.as_bytes())
            .map_err(WriteErrorKind::IO)
            .map_err(|kind| WriteError {
                path: self.path.clone(),
                kind,
            })
    }
}

/// Error loading [`SequencerConfig`] from a file
#[allow(missing_docs)]
#[non_exhaustive]
pub struct ReadError<F: FromKdlEntries> {
    pub path: std::path::PathBuf,
    pub kind: ReadErrorKind<F>,
}

/// Error kind for loading [`SequencerConfig`] from a file
#[allow(missing_docs)]
#[non_exhaustive]
pub enum ReadErrorKind<F: FromKdlEntries> {
    Parse(ParseError<F>),
    IO(std::io::Error),
}

impl<F: FromKdlEntries> std::fmt::Display for ReadError<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { path, kind } = self;
        write!(f, "for path {path:?}, {kind}")
    }
}
impl<F: FromKdlEntries> std::fmt::Display for ReadErrorKind<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse(e) => write!(f, "parse error {e:?}"),
            Self::IO(e) => write!(f, "IO error {e}"),
        }
    }
}

/// Error saving [`SequencerConfig`] to a file
#[allow(missing_docs)]
#[non_exhaustive]
pub struct WriteError<F: IntoKdlEntries> {
    pub path: std::path::PathBuf,
    pub kind: WriteErrorKind<F::Error<Never>>,
}

/// Error kind for saving [`SequencerConfig`] to a file
#[allow(missing_docs)]
#[non_exhaustive]
pub enum WriteErrorKind<E> {
    Serialize(E),
    IO(std::io::Error),
}

impl<F: IntoKdlEntries> std::fmt::Display for WriteError<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { path, kind } = self;
        write!(f, "for path {path:?}, {kind}")
    }
}
impl<E: std::fmt::Display> std::fmt::Display for WriteErrorKind<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Serialize(e) => write!(f, "serialize error {e}"),
            Self::IO(e) => write!(f, "IO error {e}"),
        }
    }
}
