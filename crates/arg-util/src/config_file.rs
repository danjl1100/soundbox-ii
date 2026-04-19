// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Provides traits for opening and writing TOML config files

pub use self::open::{ConfigFileOpen, ErrorOpen};
pub use self::open_or_write_template::ConfigOpenOrWriteTemplate;
pub use self::write::{ConfigFileWrite, ErrorWrite};

mod open {
    /// Loads the TOML configuration file for the deserializable type
    pub trait ConfigFileOpen: Sized {
        /// Loads the TOML configuration file at the specified path
        ///
        /// # Errors
        /// Reports an error if the file IO fails or the contents is not valid TOML
        fn open(path: impl AsRef<std::path::Path>) -> Result<Self, ErrorOpen>;
    }
    impl<T> ConfigFileOpen for T
    where
        T: Sized + serde::de::DeserializeOwned,
    {
        fn open(path: impl AsRef<std::path::Path>) -> Result<Self, ErrorOpen> {
            let path = path.as_ref();

            let make_error = |kind| ErrorOpen {
                path: path.to_path_buf(),
                kind,
            };

            let file_contents = std::fs::read_to_string(path)
                .map_err(ErrorOpenKind::Read)
                .map_err(make_error)?;

            toml::from_str(&file_contents)
                .map_err(ErrorOpenKind::Parse)
                .map_err(make_error)
        }
    }

    /// Error from [`ConfigFileOpen::open`]
    #[derive(Debug)]
    pub struct ErrorOpen {
        path: std::path::PathBuf,
        kind: ErrorOpenKind,
    }
    #[derive(Debug)]
    enum ErrorOpenKind {
        Read(std::io::Error),
        Parse(toml::de::Error),
    }
    impl ErrorOpen {
        /// Returns `true` if the file is missing
        #[must_use]
        pub fn is_missing_file(&self) -> bool {
            matches!(
                &self.kind,
                ErrorOpenKind::Read(err) if matches!(err.kind(), std::io::ErrorKind::NotFound)
            )
        }
    }
    impl std::error::Error for ErrorOpen {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            use ErrorOpenKind as Kind;
            match &self.kind {
                Kind::Read(error) => Some(error),
                Kind::Parse(error) => Some(error),
            }
        }
    }
    impl std::fmt::Display for ErrorOpen {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            use ErrorOpenKind as Kind;
            let Self { path, kind } = self;
            let description = match kind {
                Kind::Read(_) => "failed to read",
                Kind::Parse(_) => "failed to parse",
            };

            write!(
                f,
                "{description} config file: {path}",
                path = path.display()
            )
        }
    }
}
mod open_or_write_template {
    use crate::{
        ConfigFileOpen, ConfigFileWrite,
        config_file::{ErrorOpen, ErrorWrite},
    };

    /// Chains [`ConfigFileOpen`] or else [`ConfigFileWrite`] with the provided template value
    pub trait ConfigOpenOrWriteTemplate: Sized {
        /// Attempts to open the config file at the specified `path`
        ///
        /// If the file path does not exist, returns an error after attempting to write a template file.
        /// This template file error message uses `template_err_label` (noun) to clarify context to
        /// the user.
        /// Example `template_err_label` (noun):
        ///     - "VLC HTTP auth template file"
        ///     - "retro-encabulator cross-reluctance template file"
        ///
        /// # Errors
        /// Returns an error if the [`ConfigFileOpen`] fails, the destination template file already
        /// exists, or an error indicating that the template file was created and requires user
        /// interaction to populate the config file.
        fn open_or_write_template(
            path: &impl AsRef<std::path::Path>,
            template_err_label: &str,
            template_value_fn: impl FnOnce() -> Self,
        ) -> Result<Self, ErrorOpenWrite>;
    }
    impl<T> ConfigOpenOrWriteTemplate for T
    where
        T: serde::Serialize + serde::de::DeserializeOwned,
    {
        fn open_or_write_template(
            path: &impl AsRef<std::path::Path>,
            template_err_label: &str,
            template_value_fn: impl FnOnce() -> Self,
        ) -> Result<Self, ErrorOpenWrite> {
            let path = path.as_ref();

            let make_err = |kind| ErrorOpenWrite {
                template_err_label: template_err_label.to_string(),
                source_file: path.to_path_buf(),
                kind,
            };

            match ConfigFileOpen::open(path) {
                Ok(value) => Ok(value),
                Err(e) if e.is_missing_file() => {
                    let template_file = template_value_fn()
                        .write_template_for_file(path)
                        .map_err(Box::new)
                        .map_err(|source| ErrorKind::Write { source })
                        .map_err(make_err)?;
                    Err(make_err(ErrorKind::Created { template_file }))
                }
                Err(e) => Err(make_err(ErrorKind::Open {
                    inner_transparent: Box::new(e),
                })),
            }
        }
    }
    #[derive(Debug)]
    pub struct ErrorOpenWrite {
        template_err_label: String,
        source_file: std::path::PathBuf,
        kind: ErrorKind,
    }
    #[derive(Debug)]
    enum ErrorKind {
        Open { inner_transparent: Box<ErrorOpen> },
        Write { source: Box<ErrorWrite> },
        Created { template_file: std::path::PathBuf },
    }
    impl std::error::Error for ErrorOpenWrite {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            match &self.kind {
                ErrorKind::Open { inner_transparent } => inner_transparent.source(),
                ErrorKind::Write { source } => Some(source),
                ErrorKind::Created { template_file: _ } => None,
            }
        }
    }
    impl std::fmt::Display for ErrorOpenWrite {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let Self {
                kind,
                template_err_label,
                source_file,
            } = self;
            let source_file = source_file.display();
            match kind {
                ErrorKind::Open { inner_transparent } => write!(f, "{inner_transparent}"),
                ErrorKind::Write { source: _ } => write!(
                    f,
                    "file not found ({source_file}), then failed to create {template_err_label}",
                ),
                ErrorKind::Created { template_file } => write!(
                    f,
                    "file not found ({source_file}), created {template_err_label} at: {template_file}",
                    template_file = template_file.display(),
                ),
            }
        }
    }
}

mod write {
    /// Writes the configuration file for serializable types
    pub trait ConfigFileWrite {
        /// Writes the configuration file to for the specified path
        ///
        /// # Errors
        /// Returns an error if the write operation or serialize to TOML fail
        fn write_template_for_file(
            &self,
            path: impl AsRef<std::path::Path>,
        ) -> Result<std::path::PathBuf, ErrorWrite>;
    }
    impl<T> ConfigFileWrite for T
    where
        T: serde::Serialize,
    {
        fn write_template_for_file(
            &self,
            path: impl AsRef<std::path::Path>,
        ) -> Result<std::path::PathBuf, ErrorWrite> {
            let path = path.as_ref().to_path_buf();
            let template_file = path.with_extension("toml.template");

            let make_error = |kind| ErrorWrite {
                path: template_file.clone(),
                kind,
            };

            let default_config = self;
            let contents =
                toml::to_string(&default_config).expect("default config should serialize");

            write_create_new(&template_file, contents.as_bytes())
                .map_err(ErrorKind::from)
                .map_err(make_error)?;

            Ok(template_file)
        }
    }

    /// Like [`std::fs::write`], but uses [`std::fs::File::create_new`]
    /// instead of [`std::fs::File::create`]
    fn write_create_new(
        path: &std::path::Path,
        content: &[u8],
    ) -> Result<(), (std::io::Error, ErrorReason)> {
        use std::io::Write as _;

        let mut file = std::fs::File::create_new(path).map_err(|e| {
            use std::io::ErrorKind as Kind;
            let reason = match e.kind() {
                Kind::AlreadyExists => ErrorReason::AlreadyExists,
                _ => ErrorReason::default(),
            };
            (e, reason)
        })?;

        file.write_all(content)
            .map_err(|e| (e, ErrorReason::default()))
    }
    #[derive(Default)]
    enum ErrorReason {
        AlreadyExists,
        #[default]
        Other,
    }

    /// Error from [`ConfigFileWrite::write_template_for_file`]
    #[derive(Debug)]
    pub struct ErrorWrite {
        path: std::path::PathBuf,
        kind: ErrorKind,
    }
    #[derive(Debug)]
    enum ErrorKind {
        AlreadyExists(std::io::Error),
        Write(std::io::Error),
    }
    impl From<(std::io::Error, ErrorReason)> for ErrorKind {
        fn from((e, kind): (std::io::Error, ErrorReason)) -> Self {
            let map_fn = match kind {
                ErrorReason::AlreadyExists => Self::AlreadyExists,
                ErrorReason::Other => Self::Write,
            };
            map_fn(e)
        }
    }
    impl std::error::Error for ErrorWrite {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            match &self.kind {
                ErrorKind::AlreadyExists(error) | ErrorKind::Write(error) => Some(error),
            }
        }
    }
    impl std::fmt::Display for ErrorWrite {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let Self { path, kind } = self;
            let description = match kind {
                ErrorKind::AlreadyExists(_) => "cannot overwrite existing",
                ErrorKind::Write(_) => "failed to write",
            };
            write!(
                f,
                "{description} config file: {path}",
                path = path.display()
            )
        }
    }
}
