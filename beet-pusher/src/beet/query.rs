use crate::BeetItem;
use std::io::BufRead as _;
use std::{borrow::Cow, process::Command};
use tracing::{debug, trace};

impl BeetItem {
    /// Lists the resulting items for a beet query
    ///
    /// # Errors
    /// Returns an error if the `beet` command fails or produces invalid output
    pub fn list_from_beet_query(filters: impl Iterator<Item = String>) -> Result<Vec<Self>, Error> {
        let make_error = |kind| Error { kind };

        debug!("spawn `beet` command");

        let mut command = Command::new("beet");
        command
            //
            .arg("ls")
            .arg("-f")
            .arg("=")
            .args(filters);

        trace!(?command);

        let output = command
            .output()
            .map_err(ErrorKind::Spawn)
            .map_err(make_error)?;

        if !output.stderr.is_empty() {
            return Err(make_error(ErrorKind::Stderr {
                stderr_str: String::from_utf8_lossy(&output.stderr).to_string(),
            }));
        }

        if !output.status.success() {
            return Err(make_error(ErrorKind::ExitFail {
                code: output.status.code(),
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            }));
        }

        debug!("parse `beet` output ({} bytes)", output.stdout.len());

        // let output_len = output.stdout.len();
        // let output_trim = String::from_utf8_lossy(if output_len > 100 {
        //     &output.stdout[0..100]
        // } else {
        //     &output.stdout[..]
        // });
        // trace!(?output_trim, ?output_len);

        output
            .stdout
            .lines()
            .map(|line| {
                let line = line.map_err(ErrorKind::Read)?;
                line.parse()
                    .map_err(|error| ErrorKind::InvalidLine { line, error })
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(make_error)
    }
}

#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
}
#[derive(Debug)]
enum ErrorKind {
    Spawn(std::io::Error),
    Read(std::io::Error),
    Stderr {
        stderr_str: String,
    },
    ExitFail {
        code: Option<i32>,
        stdout: String,
    },
    InvalidLine {
        line: String,
        error: super::item::Error,
    },
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        use ErrorKind as E;
        match &self.kind {
            E::Spawn(error) | E::Read(error) => Some(error),
            E::Stderr { stderr_str: _ } | E::ExitFail { .. } => None,
            E::InvalidLine { error, .. } => Some(error),
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        #[rustfmt::skip]
        fn funnel<'a, F: Fn(usize, &'a str) -> Cow<'a, str>>(f: F) -> F { f }
        let max_len = funnel(|max, raw_input: &str| {
            if raw_input.len() > max {
                Cow::Owned(format!("{} ...", &raw_input[..max]))
            } else {
                Cow::Borrowed(raw_input)
            }
        });
        let (description, details) = match &self.kind {
            ErrorKind::Spawn(_) => ("failed to spawn", None),
            ErrorKind::Read(_) => ("failed to read from", None),
            ErrorKind::Stderr { stderr_str } => {
                ("stderr output from", Some(Cow::Borrowed(&**stderr_str)))
            }
            ErrorKind::ExitFail { code, stdout } => {
                let stdout = max_len(200, stdout);
                let details = match code {
                    Some(code) => Cow::Owned(format!("[code {code}] {stdout}")),
                    None => stdout,
                };
                ("failure status code from", Some(details))
            }
            ErrorKind::InvalidLine { line, error: _ } => {
                let line = max_len(80, line);
                ("invalid output line from", Some(line))
            }
        };
        write!(f, "{description} beet command")?;
        if let Some(details) = details {
            write!(f, ": {details}")?;
        }
        Ok(())
    }
}
