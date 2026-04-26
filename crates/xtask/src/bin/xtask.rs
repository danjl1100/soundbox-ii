// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Helper commands for the repo, following the
//! [`cargo-xtask`](https://github.com/matklad/cargo-xtask/) template

use clap::Parser as _;
use xtask::spigot_visual::HintAllowRustWorkspaceCalls;
use xtask::{CmdSettings, TypedResult, WriteOutput};

#[derive(Debug, clap::Parser)]
struct Args {
    #[clap(subcommand)]
    subcommand: Subcommand,
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
        Subcommand::Checks(checks) => checks.all_checks()?,
        Subcommand::SpigotVisualRun(run) => run.run()?,
        Subcommand::SpigotVisualDist => {
            let cmd = &CmdSettings::new(xtask::Verbosity::default());
            let write = WriteOutput::unchecked_user_wants_to_write_files();
            xtask::spigot_visual::DistJs::dist_js(cmd, write)?;
        }
        Subcommand::Vlc(run_web) => run_web.run_web()?,
    }
    Ok(())
}

/// Runs all linting checks (cargo-vet, -fmt, -clippy, -doc, -test, JS linter, and copyright notes)
#[derive(Debug, clap::Args)]
struct AllChecks {
    #[clap(flatten)]
    cmd_args: xtask::ArgsCmdSettings,
    #[clap(flatten)]
    fix: xtask::ArgFix,
}
impl AllChecks {
    fn all_checks(self) -> TypedResult<()> {
        let AllChecks { cmd_args, fix } = self;
        let fix = fix.into_inner();

        let cmd = &cmd_args.into_inner();

        let hint = HintAllowRustWorkspaceCalls::check_and_run_once(cmd, fix)?;

        xtask::copyright::checks(cmd, fix)?;
        xtask::supply_chain::checks(cmd, fix, &hint)?;
        xtask::rust::checks(cmd, fix, &hint)?;
        xtask::spigot_visual::checks(cmd, fix)?;

        Ok(())
    }
}
