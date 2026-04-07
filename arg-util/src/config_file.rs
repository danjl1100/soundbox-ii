// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Provides traits for opening and writing TOML config files

pub use self::open::{ConfigFileOpen, ErrorOpen};
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

            std::fs::write(&template_file, contents.as_bytes())
                .map_err(ErrorWriteKind::Write)
                .map_err(make_error)?;

            Ok(template_file)
        }
    }

    /// Error from [`ConfigFileWrite::write_template_for_file`]
    #[derive(Debug)]
    pub struct ErrorWrite {
        path: std::path::PathBuf,
        kind: ErrorWriteKind,
    }
    #[derive(Debug)]
    enum ErrorWriteKind {
        Write(std::io::Error),
    }
    impl std::error::Error for ErrorWrite {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            match &self.kind {
                ErrorWriteKind::Write(error) => Some(error),
            }
        }
    }
    impl std::fmt::Display for ErrorWrite {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let Self { path, kind } = self;
            let description = match kind {
                ErrorWriteKind::Write(_) => "failed to write",
            };
            write!(
                f,
                "{description} config file: {path}",
                path = path.display()
            )
        }
    }
}
