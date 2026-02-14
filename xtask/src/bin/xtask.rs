// Copyright (C) 2021-2025  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Helper commands for the repo, following the
//! [`cargo-xtask`](https://github.com/matklad/cargo-xtask/) template

use xtask::spigot_visual::HintAllowRustWorkspaceCalls;
use xtask::{Fix, TypedResult, WriteOutput};

const HELP_TEXT: &str = "Tasks:

checks [fix]            run all linting checks
spigot-visual-run       runs spigot-visual with the compiled typescript
spigot-visual-dist      compiles the spigot-visual typescript
vlc                     runs VLC with required arguments for the web interface
";

fn main() -> eyre::Result<()> {
    #[expect(clippy::redundant_closure_for_method_calls)]
    main_inner().map_err(|e| e.into_eyre_in_final_main_error_report_location())
}
fn main_inner() -> TypedResult<()> {
    let mut args = std::env::args().skip(1);
    let task = args.next();
    match task.as_deref() {
        Some("checks") => all_checks(args)?,
        Some("spigot-visual-run") => xtask::spigot_visual::run(args)?,
        Some("spigot-visual-dist") => xtask::spigot_visual::dist_js(Some(WriteOutput))?,
        Some("vlc") => xtask::vlc::run_web(args)?,
        _ => print_help(),
    }
    Ok(())
}

fn all_checks(mut args: impl Iterator<Item = String>) -> TypedResult<()> {
    let bail_unknown = |arg| eyre::eyre!("unknown checks argument: {arg:?}");
    let fix = args
        .next()
        .map(|arg| {
            if arg == "fix" {
                Ok(Fix)
            } else {
                Err(bail_unknown(arg))
            }
        })
        .transpose()?;

    if let Some(extra) = args.next() {
        return Err(bail_unknown(extra).into());
    }

    let hint = HintAllowRustWorkspaceCalls::check_and_run_once(fix)?;

    xtask::copyright::checks(fix)?;
    xtask::rust::checks(fix, &hint)?;
    xtask::spigot_visual::checks(fix)?;

    Ok(())
}

fn print_help() {
    eprintln!("{HELP_TEXT}");
}
