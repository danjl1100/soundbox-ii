// Copyright (C) 2021-2025  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Helper commands for the repo, following the
//! [`cargo-xtask`](https://github.com/matklad/cargo-xtask/) template

use eyre::Context as _;
use std::{
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
};

const HELP_TEXT: &str = "Tasks:

checks [fix]            run all linting checks
spigot-visual-run       runs spigot-visual with the compiled typescript
spigot-visual-dist      compiles the spigot-visual typescript
";

fn main() -> eyre::Result<()> {
    let mut args = std::env::args().skip(1);
    let task = args.next();
    match task.as_deref() {
        Some("checks") => all_checks(args)?,
        Some("spigot-visual-run") => spigot_visual::run(args)?,
        Some("spigot-visual-dist") => spigot_visual::dist_js(Some(WriteOutput))?,
        _ => print_help(),
    }
    Ok(())
}

/// If present, attempt to fix the checks by writing to files (otherwise, run read-only checks)
#[derive(Clone, Copy, Debug)]
struct Fix;

/// If present, write output files (otherwise, run read-only checks)
#[derive(Clone, Copy, Debug)]
struct WriteOutput;

fn all_checks(mut args: impl Iterator<Item = String>) -> eyre::Result<()> {
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
        return Err(bail_unknown(extra));
    }

    rust::checks(fix)?;
    spigot_visual::checks(fix)?;

    Ok(())
}

fn run_cargo(args_fn: impl FnOnce(&mut Command) -> &mut Command) -> eyre::Result<()> {
    let status = status_cargo(args_fn)?;
    if !status.success() {
        eyre::bail!("cargo command failed");
    }
    Ok(())
}
fn status_cargo(args_fn: impl FnOnce(&mut Command) -> &mut Command) -> eyre::Result<ExitStatus> {
    status_cmd(env!("CARGO"), args_fn)
}
fn run_cmd(cmd: &str, args_fn: impl FnOnce(&mut Command) -> &mut Command) -> eyre::Result<()> {
    let status = status_cmd(cmd, args_fn)?;
    if !status.success() {
        // dbg!(&command);
        eyre::bail!("`{cmd}` failed");
    }

    Ok(())
}
fn status_cmd(
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

fn print_help() {
    eprintln!("{HELP_TEXT}");
}

fn print_help_fix_checks() {
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

fn project_root() -> PathBuf {
    Path::new(&env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(1)
        .expect("CARGO_MANIFEST_DIR should have at least one ancestor")
        .to_path_buf()
}

mod rust {
    use crate::{Fix, run_cargo, status_cargo};

    pub fn checks(fix: Option<Fix>) -> eyre::Result<()> {
        fmt(fix)?;
        clippy(fix)?;
        test()?;
        doc()?;

        Ok(())
    }

    pub fn fmt(fix: Option<Fix>) -> eyre::Result<()> {
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
            eyre::bail!("cargo fmt failed");
        }
        Ok(())
    }

    fn clippy(fix: Option<Fix>) -> eyre::Result<()> {
        run_cargo(|c| {
            c.args([
                "clippy",
                "--workspace",
                "--all-targets",
                "--color",
                "always",
            ]);
            if let Some(Fix) = fix {
                c.arg("--fix");
            }
            c
        })
    }
    fn test() -> eyre::Result<()> {
        run_cargo(|c| c.args(["test", "--workspace", "--color", "always"]))
    }
    fn doc() -> eyre::Result<()> {
        run_cargo(|c| c.args(["doc", "--workspace", "--no-deps", "--color", "always"]))
    }
}

mod spigot_visual {
    //! Tasks for the `spigot_visual` crate

    use crate::{Fix, WriteOutput, print_help_fix_checks, project_root, run_cargo, run_cmd};
    use std::{ffi::OsStr, path::PathBuf};

    pub fn run<S>(args: impl IntoIterator<Item = S>) -> eyre::Result<()>
    where
        S: AsRef<OsStr>,
    {
        fmt_js()?;
        dist_js(Some(WriteOutput))?;

        run_cargo(|c| {
            c.args(["run", "--package", "spigot-visual", "--"])
                //
                .arg("--dev-path-prefix")
                .arg(dist_dir())
                //
                .args(args)
        })
    }

    pub fn checks(fix: Option<Fix>) -> eyre::Result<()> {
        check_js(fix)?;
        dist_js(None)?;
        Ok(())
    }
    pub fn check_js(fix: Option<Fix>) -> eyre::Result<()> {
        let result = run_cmd("biome", |c| {
            c.arg("check");
            if let Some(Fix) = fix {
                c.arg("--write");
            }
            c.current_dir(ts_src_dir())
        });
        if result.is_err() && fix.is_none() {
            print_help_fix_checks();
        }
        result
    }

    pub fn fmt_js() -> eyre::Result<()> {
        // `biome format --write` also works, but format is a subset of `biome check --write`
        check_js(Some(Fix))
    }

    pub fn dist_js(write: Option<WriteOutput>) -> eyre::Result<()> {
        run_cmd("tsc", |c| {
            if write.is_none() {
                c.arg("--noEmit");
            }
            c.current_dir(ts_src_dir())
        })
    }

    fn ts_src_dir() -> PathBuf {
        project_root().join("spigot-visual/static-ts")
    }
    fn dist_dir() -> PathBuf {
        project_root().join("spigot-visual/static")
    }
}
