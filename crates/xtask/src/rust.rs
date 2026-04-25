// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Checks for the rust source code as a whole

use crate::{CmdSettings, TypedResult, WriteOutput, spigot_visual::HintAllowRustWorkspaceCalls};

/// Checks the rust source as a whole
///
/// # Errors
/// Returns an error if any subprocesses fail
pub fn checks(
    cmd: &CmdSettings,
    fix: Option<WriteOutput>,
    _hint: &HintAllowRustWorkspaceCalls,
) -> TypedResult<()> {
    fmt(cmd, fix)?;
    clippy(cmd, fix)?;
    test(cmd)?;
    doc(cmd)?;

    Ok(())
}

fn fmt(cmd: &CmdSettings, fix: Option<WriteOutput>) -> TypedResult<()> {
    if fix.is_none() {
        // no fix = printing list
        eprintln!("Outstanding cargo fmt files:");
    }

    let status = cmd.status_cargo(|c| {
        c.args(["fmt", "--all", "--"]);
        if fix.is_none() {
            c.args(["--check", "-l"]);
        }
        c
    })?;
    if fix.is_none() {
        // no fix = printing list
        if status.success() {
            // passed, report "none"
            eprintln!("[none]");
        }
        eprintln!("{:-<80}", "");
    }

    if !status.success() {
        crate::bail!("cargo fmt failed")
    }
    Ok(())
}

fn clippy(cmd: &CmdSettings, fix: Option<WriteOutput>) -> TypedResult<()> {
    cmd.run_cargo(|c| {
        c.args([
            "clippy",
            "--workspace",
            "--all-targets",
            "--color",
            "always",
        ]);
        if let Some(WriteOutput { .. }) = fix {
            c.args(["--fix", "--allow-dirty"]);
        }
        c
    })
}
fn test(cmd: &CmdSettings) -> TypedResult<()> {
    cmd.run_cargo(|c| c.args(["test", "--workspace", "--color", "always"]))
}
fn doc(cmd: &CmdSettings) -> TypedResult<()> {
    cmd.run_cargo(|c| c.args(["doc", "--workspace", "--no-deps", "--color", "always"]))
}
