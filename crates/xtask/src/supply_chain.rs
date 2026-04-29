// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Checks audits for the rust dependencies

use crate::{CmdSettings, TypedResult, WriteOutput};

/// Checks audits for the rust dependencies via `cargo-vet`
///
/// NOTE: [`WriteOutput`] here doesn't fix the dependencies or run audits, instead it allows
/// `cargo-vet` to fetch audit sources and update the lockfile
///
/// # Errors
/// Returns an error if any subprocesses fail
pub fn checks(cmd: &CmdSettings, fix: Option<WriteOutput>) -> TypedResult<()> {
    let status = cmd.status_cargo(|c| {
        c.args(["vet", "check"]);
        if fix.is_none() {
            c.args(["--locked", "--frozen"]);
        }
        c
    })?;

    if !status.success() {
        crate::bail!("cargo vet failed")
    }
    Ok(())
}
