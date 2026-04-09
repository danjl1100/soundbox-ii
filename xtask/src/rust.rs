// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Checks for the rust source code as a whole

use crate::{
    Fix, TypedResult, run_cargo, spigot_visual::HintAllowRustWorkspaceCalls, status_cargo,
};

/// Checks the rust source as a whole
///
/// # Errors
/// Returns an error if any subprocesses fail
pub fn checks(fix: Option<Fix>, _hint: &HintAllowRustWorkspaceCalls) -> TypedResult<()> {
    fmt(fix)?;
    clippy(fix)?;
    test()?;
    doc()?;

    Ok(())
}

fn fmt(fix: Option<Fix>) -> TypedResult<()> {
    if fix.is_none() {
        // no fix = printing list
        eprintln!("Outstanding cargo fmt files:");
    }

    let status = status_cargo(|c| {
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

fn clippy(fix: Option<Fix>) -> TypedResult<()> {
    run_cargo(|c| {
        c.args([
            "clippy",
            "--workspace",
            "--all-targets",
            "--color",
            "always",
        ]);
        if let Some(Fix::Fix) = fix {
            c.args(["--fix", "--allow-dirty"]);
        }
        c
    })
}
fn test() -> TypedResult<()> {
    run_cargo(|c| c.args(["test", "--workspace", "--color", "always"]))
}
fn doc() -> TypedResult<()> {
    run_cargo(|c| c.args(["doc", "--workspace", "--no-deps", "--color", "always"]))
}
