//! Test-double for `beet`, providing tunable stdout/stderr, delay, and exit code based on the
//! input arguments

use std::{collections::BTreeMap, process::ExitCode};

mod serde;

/// Configuration for how `fake-beet` should react to various provided argument sequences
#[derive(Debug, Default, ::serde::Serialize, ::serde::Deserialize)]
pub struct ConfigAll {
    /// Map from JSON-array to the config
    #[serde(serialize_with = "self::serde::serialize_map_keys_as_json")]
    #[serde(deserialize_with = "self::serde::deserialize_map_keys_as_json")]
    input_args: BTreeMap<Vec<String>, ConfigOut>,
}
/// `fake-beet` output parameters for a specific arguments list
#[derive(Debug, Default, ::serde::Serialize, ::serde::Deserialize)]
pub struct ConfigOut {
    stdout: String,
    stderr: String,
    exit_code: u8,
    delay_millis: u16,
}
/// Publicly visible version of [`ConfigOut`]
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
    /// Sets the string output to stdout
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
    /// Sets the string output to stderr
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
