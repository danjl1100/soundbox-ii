// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! On Unix, replace the current process instead of spawning a subprocess

use crate::DisplayCommand;
use eyre::Context as _;
use std::process::Command;

/// Runs the specified `cargo` command, replacing the current process
#[expect(clippy::missing_errors_doc, reason = "infallible")]
pub fn exec_cargo(
    args_fn: impl FnOnce(&mut Command) -> &mut Command,
) -> eyre::Result<std::convert::Infallible> {
    exec_cmd(env!("CARGO"), args_fn)
}
/// Runs the specified command, replacing the current process
#[expect(clippy::missing_errors_doc, reason = "infallible")]
pub fn exec_cmd(
    cmd: &str,
    args_fn: impl FnOnce(&mut Command) -> &mut Command,
) -> eyre::Result<std::convert::Infallible> {
    exec_cmd_try_args(cmd, |c| Ok(args_fn(c)))
        .unwrap_or_else(|never: std::convert::Infallible| match never {})
}
/// Runs the specified command, replacing the current process
#[expect(clippy::missing_errors_doc, reason = "infallible")]
pub fn exec_cmd_try_args<E>(
    cmd: &str,
    args_fn: impl FnOnce(&mut Command) -> Result<&mut Command, E>,
) -> Result<eyre::Result<std::convert::Infallible>, E> {
    use std::os::unix::process::CommandExt as _;

    let mut command = Command::new(cmd);
    args_fn(&mut command)?;

    eprintln!("{}", DisplayCommand(&command));

    let err = command.exec();
    Ok(Err(err).with_context(|| {
        dbg!(&command);
        format!("failed to run `{cmd}`")
    }))
}
