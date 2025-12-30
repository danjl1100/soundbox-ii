// Copyright (C) 2021-2025  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! On Unix, replace the current process instead of spawning a subprocess

use eyre::Context as _;
use std::process::Command;

/// Runs the specified `cargo` command, replacing the current process
#[expect(clippy::missing_errors_doc)] // infallible
pub fn exec_cargo(
    args_fn: impl FnOnce(&mut Command) -> &mut Command,
) -> eyre::Result<std::convert::Infallible> {
    exec_cmd(env!("CARGO"), args_fn)
}
/// Runs the specified command, replacing the current process
#[expect(clippy::missing_errors_doc)] // infallible
pub fn exec_cmd(
    cmd: &str,
    args_fn: impl FnOnce(&mut Command) -> &mut Command,
) -> eyre::Result<std::convert::Infallible> {
    use std::os::unix::process::CommandExt as _;

    let mut command = Command::new(cmd);
    args_fn(&mut command);
    let err = command.exec();
    Err(err).with_context(|| {
        dbg!(&command);
        format!("failed to run `{cmd}`")
    })
}
