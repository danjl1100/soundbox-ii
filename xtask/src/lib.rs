// Copyright (C) 2021-2025  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Logic for the `xtask` functionality

use eyre::Context as _;
use std::{
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
};

pub mod copyright;
pub mod rust;
pub mod spigot_visual;

#[cfg(unix)]
pub mod unix_exec;

/// If present, attempt to fix the checks by writing to files (otherwise, run read-only checks)
#[derive(Clone, Copy, Debug)]
pub struct Fix;

/// If present, write output files (otherwise, run read-only checks)
#[derive(Clone, Copy, Debug)]
pub struct WriteOutput;

/// Runs the specified `cargo` command
///
/// # Errors
/// Returns an error if the command fails
pub fn run_cargo(args_fn: impl FnOnce(&mut Command) -> &mut Command) -> eyre::Result<()> {
    let status = status_cargo(args_fn)?;
    if !status.success() {
        eyre::bail!("cargo command failed");
    }
    Ok(())
}
/// Statuses the specified `cargo` command
///
/// # Errors
/// Returns an error if the command fails
pub fn status_cargo(
    args_fn: impl FnOnce(&mut Command) -> &mut Command,
) -> eyre::Result<ExitStatus> {
    status_cmd(env!("CARGO"), args_fn)
}
/// Runs the specified command
///
/// # Errors
/// Returns an error if the command fails
pub fn run_cmd(cmd: &str, args_fn: impl FnOnce(&mut Command) -> &mut Command) -> eyre::Result<()> {
    let status = status_cmd(cmd, args_fn)?;
    if !status.success() {
        // dbg!(&command);
        eyre::bail!("`{cmd}` failed");
    }

    Ok(())
}
/// Statuses the specified command
///
/// # Errors
/// Returns an error if the command fails
pub fn status_cmd(
    cmd: &str,
    args_fn: impl FnOnce(&mut Command) -> &mut Command,
) -> eyre::Result<ExitStatus> {
    let mut command = Command::new(cmd);
    args_fn(&mut command);
    command.status().with_context(|| {
        dbg!(&command);
        format!("failed to run `{cmd}`")
    })
}

/// Prints the help note for applying fixes
pub fn print_help_fix_checks() {
    let args = ["cargo xtask".to_owned()]
        .into_iter()
        .chain(std::env::args().skip(1))
        .chain(["fix".to_string()])
        .fold(String::new(), |mut acc, arg| {
            use std::fmt::Write as _;
            if !acc.is_empty() {
                write!(acc, " ").expect("infallible");
            }
            write!(acc, "{arg}").expect("infallible");
            acc
        });
    eprintln!("NOTE: Apply fixes using `{args}`");
}

/// Returns the root project folder path
///
/// # Panics
/// Panics at runtime if compiled with an invalid `CARGO_MANIFEST_DIR`
#[must_use]
pub fn project_root() -> PathBuf {
    Path::new(&env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(1)
        .expect("CARGO_MANIFEST_DIR should have at least one ancestor")
        .to_path_buf()
}
