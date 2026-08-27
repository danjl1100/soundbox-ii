// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Test-double for `beet`, providing tunable stdout/stderr, delay, and exit code based on the
//! input arguments

use eyre::Context as _;
use std::{collections::BTreeMap, process::ExitCode};

mod serde;

/// Environment variable required for the file written from [`ConfigAll::create_config_file`]
pub const FAKE_BEET_CONFIG_FILE: &str = "FAKE_BEET_CONFIG_FILE";

/// Builds the binary **that is part of the current workspace**
pub fn try_build_bin_once() -> &'static escargot::error::CargoResult<escargot::CargoRun> {
    use escargot::{CargoBuild, CargoRun, error::CargoResult};
    use std::sync::OnceLock;

    static BIN: OnceLock<CargoResult<CargoRun>> = OnceLock::new();
    BIN.get_or_init(|| {
        CargoBuild::new()
            .bin("fake-beet")
            .package("fake-beet")
            .current_release()
            .run()
    })
}

/// Builds the binary **that is part of the current workspace**
///
/// # Panics
/// Panics if the cargo invocation fails
#[must_use]
pub fn build_bin_once() -> &'static escargot::CargoRun {
    try_build_bin_once()
        .as_ref()
        .expect("failed to build fake-beet helper binary")
}

/// Entrypoint for `fake-beet`
///
/// NOTE: the `fake-beet` binary might need to be replicated to use in multiple crates' tests
///
/// # Errors
/// Returns an error if loading the configuration file fails
pub fn fake_beet_main() -> eyre::Result<ExitCode> {
    let config_file = {
        let var = FAKE_BEET_CONFIG_FILE;
        std::env::var(var).with_context(|| format!("missing required fake-beet env var: {var}"))
    }?;
    let config_str = std::fs::read_to_string(&config_file)
        .with_context(|| format!("invalid fake-beet config file path: {config_file}"))?;

    let config_all: ConfigAll = serde_json::from_str(&config_str)
        .with_context(|| format!("invalid fake-beet config: {config_str:?}"))?;

    let args: Vec<_> = std::env::args().skip(1).collect();
    let Some(config) = config_all.into_configs_map().remove(&args) else {
        eyre::bail!("unknown fake-beet args: {args:?}");
    };

    let exit_code = config.execute();
    Ok(exit_code)
}

/// Configuration for how `fake-beet` should react to various provided argument sequences
#[derive(Debug, Default, PartialEq, Eq, ::serde::Serialize, ::serde::Deserialize)]
pub struct ConfigAll {
    /// Map from JSON-array to the config
    #[serde(serialize_with = "self::serde::serialize_map_keys_as_json")]
    #[serde(deserialize_with = "self::serde::deserialize_map_keys_as_json")]
    input_args: BTreeMap<Vec<String>, ConfigOut>,
}
/// `fake-beet` output parameters for a specific arguments list
#[derive(Debug, Default, PartialEq, Eq, ::serde::Serialize, ::serde::Deserialize)]
pub struct ConfigOut {
    stdout: String,
    stderr: String,
    exit_code: u8,
    delay_millis: u16,
}
/// Publicly readable output from [`ConfigOut::into_output`]
#[expect(missing_docs, reason = "self-explanatory field names")]
pub struct ConfigOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: u8,
    pub delay_millis: u16,
}

impl ConfigOut {
    /// Runs the configuration:
    /// - Sleeps for `delay_millis`
    /// - Writes `stdout`
    /// - Writes `stderr`
    /// - Returns the `exit_code`
    #[must_use]
    pub fn execute(self) -> ExitCode {
        let Self {
            stdout,
            stderr,
            exit_code,
            delay_millis,
        } = self;

        std::thread::sleep(std::time::Duration::from_millis(delay_millis.into()));

        print!("{stdout}");
        eprint!("{stderr}");

        ExitCode::from(exit_code)
    }
    /// Returns a readable format of the configuration
    #[must_use]
    pub fn into_output(self) -> ConfigOutput {
        let Self {
            stdout,
            stderr,
            exit_code,
            delay_millis,
        } = self;
        ConfigOutput {
            stdout,
            stderr,
            exit_code,
            delay_millis,
        }
    }
}

impl ConfigAll {
    /// Returns a default config with the specified setup function applied
    pub fn setup_with(setup_fn: impl FnOnce(&mut Self)) -> Self {
        let mut config = Self::default();
        setup_fn(&mut config);
        config
    }
    /// Returns the map of all configured outputs
    #[must_use]
    pub fn into_configs_map(self) -> BTreeMap<Vec<String>, ConfigOut> {
        let Self { input_args } = self;
        input_args
    }
    /// Adds a new default [`ConfigOut`] for the empty args
    ///
    /// (helper for [`ConfigAll::for_args`] with empty input)
    pub fn for_args_empty(&mut self, empty_args: [&'static str; 0]) -> &mut ConfigOut {
        self.for_args(empty_args)
    }
    /// Adds a new default [`ConfigOut`] for the specified args
    ///
    /// # Panics
    /// Panics if the arguments are already defined
    #[expect(clippy::panic, reason = "test-only crate should report error")]
    pub fn for_args(&mut self, args: impl IntoIterator<Item: std::fmt::Display>) -> &mut ConfigOut {
        use std::collections::btree_map::Entry;

        let args = args.into_iter().map(|v| v.to_string()).collect();

        let config_new = ConfigOut::default();

        match self.input_args.entry(args) {
            Entry::Vacant(vacant_entry) => vacant_entry.insert(config_new),
            Entry::Occupied(occupied_entry) => {
                let key = occupied_entry.key();
                let config_old = occupied_entry.get();
                panic!("duplicate entries for args {key:?}: {config_old:?} and {config_new:?}")
            }
        }
    }
}
impl ConfigOut {
    /// Sets the stdout string output
    ///
    /// # Panics
    /// Panics if stdout was already set
    #[track_caller]
    pub fn stdout(&mut self, stdout_new: impl std::fmt::Display) -> &mut Self {
        let Self { stdout, .. } = self;
        assert!(stdout.is_empty(), "duplicate: {stdout} and {stdout_new}");
        *stdout = stdout_new.to_string();
        self
    }
    /// Sets the stdout string output based on the input lines (no extra trailing newline)
    ///
    /// # Panics
    /// Panics if stdout was already set
    #[track_caller]
    pub fn stdout_lines(
        &mut self,
        stdout_lines: impl IntoIterator<Item: std::fmt::Display>,
    ) -> &mut Self {
        self.stdout(lines(stdout_lines))
    }
    /// Sets the stderr string output
    ///
    /// # Panics
    /// Panics if stderr was already set
    #[track_caller]
    pub fn stderr(&mut self, stderr_new: impl std::fmt::Display) -> &mut Self {
        let Self { stderr, .. } = self;
        assert!(stderr.is_empty(), "duplicate: {stderr} and {stderr_new}");
        *stderr = stderr_new.to_string();
        self
    }
    /// Sets the stderr string output based on the input lines (no extra trailing newline)
    ///
    /// # Panics
    /// Panics if stderr was already set
    #[track_caller]
    pub fn stderr_lines(
        &mut self,
        stderr_lines: impl IntoIterator<Item: std::fmt::Display>,
    ) -> &mut Self {
        self.stderr(lines(stderr_lines))
    }
    /// Sets the exit code
    ///
    /// # Panics
    /// Panics if the exit code was already set
    #[track_caller]
    pub fn exit_code(&mut self, exit_code_new: u8) -> &mut Self {
        let Self { exit_code, .. } = self;
        assert!(
            *exit_code == 0,
            "duplicate: {exit_code} and {exit_code_new}"
        );
        *exit_code = exit_code_new;
        self
    }
    /// Sets the delay in milliseconds
    ///
    /// # Panics
    /// Panics if the delay was already set
    pub fn delay_millis(&mut self, delay_millis_new: u16) -> &mut Self {
        let Self { delay_millis, .. } = self;
        assert!(
            *delay_millis == 0,
            "duplicate {delay_millis} and {delay_millis_new}"
        );
        *delay_millis = delay_millis_new;
        self
    }
}

fn lines(lines: impl IntoIterator<Item: std::fmt::Display>) -> String {
    lines.into_iter().fold(String::new(), |mut acc, line| {
        use std::fmt::Write as _;
        if !acc.is_empty() {
            writeln!(&mut acc).expect("infallible");
        }
        write!(&mut acc, "{line}").expect("infallible");
        acc
    })
}

impl ConfigAll {
    /// Writes the config file content in the specified folder and filename, returning the
    /// complete path
    ///
    /// # Errors
    /// Returns an error if writing the file fails
    pub fn create_config_file(
        &self,
        dir: &std::path::Path,
        file_name: &str,
    ) -> eyre::Result<std::path::PathBuf> {
        let file_content = serde_json::to_string_pretty(self)
            .context("failed to serialize fake_beet::ConfigAll")?;
        create_config_file(dir, file_name, &file_content)
    }
}

/// Writes the config file content in the specified folder and filename, returning the complete path
///
/// # Errors
/// Returns an error if writing the file fails
pub fn create_config_file(
    dir: &std::path::Path,
    file_name: &str,
    file_content: &str,
) -> eyre::Result<std::path::PathBuf> {
    let mut p = dir.to_path_buf();
    p.push(file_name);
    std::fs::write(&p, file_content)
        .with_context(|| format!("failed to create {file_name} at {}", p.display()))?;

    Ok(p)
}
