// Copyright (C) 2021-2025  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Helper commands for the repo, following the
//! [`cargo-xtask`](https://github.com/matklad/cargo-xtask/) template

use eyre::Context as _;
use std::{
    path::{Path, PathBuf},
    process::Command,
};

fn main() -> eyre::Result<()> {
    let mut args = std::env::args().skip(1);
    let task = args.next();
    match task.as_deref() {
        Some("spigot-visual-run") => spigot_visual::run(args)?,
        Some("spigot-visual-dist") => spigot_visual::dist_js()?,
        _ => print_help(),
    }
    Ok(())
}

fn run_cmd(cmd: &str, args_fn: impl FnOnce(&mut Command) -> &mut Command) -> eyre::Result<()> {
    let mut command = Command::new(cmd);
    args_fn(&mut command);
    let status = command.status().with_context(|| {
        dbg!(&command);
        format!("failed to run `{cmd}`")
    })?;

    if !status.success() {
        dbg!(&command);
        eyre::bail!("`{cmd}` failed");
    }

    Ok(())
}

fn print_help() {
    eprintln!(
        "Tasks:

spigot-visual-run       runs spigot-visual with the compiled typescript
spigot-visual-dist      compiles the spigot-visual typescript
"
    );
}

fn project_root() -> PathBuf {
    Path::new(&env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(1)
        .expect("CARGO_MANIFEST_DIR should have at least one ancestor")
        .to_path_buf()
}

mod spigot_visual {
    //! Tasks for the `spigot_visual` crate

    use crate::{project_root, run_cmd};
    use std::{ffi::OsStr, path::PathBuf};

    pub fn run<S>(args: impl IntoIterator<Item = S>) -> eyre::Result<()>
    where
        S: AsRef<OsStr>,
    {
        fmt_js()?;
        dist_js()?;

        run_cmd(env!("CARGO"), |c| {
            c.args(["run", "--package", "spigot-visual", "--"])
                //
                .arg("--dev-path-prefix")
                .arg(dist_dir())
                //
                .args(args)
        })
    }

    pub fn fmt_js() -> eyre::Result<()> {
        run_cmd("biome", |c| {
            c.args(["format", "--write"]).current_dir(ts_src_dir())
        })
    }

    pub fn dist_js() -> eyre::Result<()> {
        run_cmd("tsc", |c| c.current_dir(ts_src_dir()))
    }

    fn ts_src_dir() -> PathBuf {
        project_root().join("spigot-visual/static-ts")
    }
    fn dist_dir() -> PathBuf {
        project_root().join("spigot-visual/static")
    }
}
