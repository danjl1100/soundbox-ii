// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Helper commands for the repo, following the
//! [`cargo-xtask`](https://github.com/matklad/cargo-xtask/) template

use clap::Parser as _;
use xtask::spigot_visual::HintAllowRustWorkspaceCalls;
use xtask::{Fix, TypedResult};

const HELP_TEXT: &str = "Tasks:

checks [fix]            run all linting checks
spigot-visual-run       runs spigot-visual with the compiled typescript
spigot-visual-dist      compiles the spigot-visual typescript
vlc                     runs VLC with required arguments for the web interface
";

#[derive(Debug, clap::Parser)]
struct Args {
    #[clap(subcommand)]
    subcommand: Option<Subcommand>,
}
#[derive(Debug, clap::Subcommand)]
enum Subcommand {
    Checks(AllChecks),
    SpigotVisualRun(xtask::spigot_visual::Run),
    /// Compiles the spigot-visual typescript
    SpigotVisualDist,
    Vlc(xtask::vlc::RunWeb),
}

fn main() -> eyre::Result<()> {
    main_inner().map_err(xtask::TypedErr::into_eyre_in_final_main_error_report_location)
}
fn main_inner() -> TypedResult<()> {
    let Args { subcommand } = Args::parse();
    match subcommand {
        Some(Subcommand::Checks(checks)) => checks.all_checks()?,
        Some(Subcommand::SpigotVisualRun(run)) => run.run()?,
        Some(Subcommand::SpigotVisualDist) => xtask::spigot_visual::DistJs::default().dist_js()?,
        Some(Subcommand::Vlc(run_web)) => run_web.run_web()?,
        None => print_help(),
    }
    Ok(())
}

/// Runs all linting checks
#[derive(Debug, clap::Args)]
struct AllChecks {
    #[clap(subcommand)]
    fix: Option<Fix>,
}
impl AllChecks {
    fn all_checks(self) -> TypedResult<()> {
        let AllChecks { fix } = self;

        let hint = HintAllowRustWorkspaceCalls::check_and_run_once(fix)?;

        xtask::copyright::checks(fix)?;
        xtask::rust::checks(fix, &hint)?;
        xtask::spigot_visual::checks(fix)?;

        Ok(())
    }
}

fn print_help() {
    eprintln!("{HELP_TEXT}");
}
