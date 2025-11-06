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
#[cfg(unix)]
mod unix_exec {
    //! On Unix, replace the current process instead of spawning a subprocess

    use eyre::Context as _;
    use std::process::Command;

    pub fn exec_run_cargo(
        args_fn: impl FnOnce(&mut Command) -> &mut Command,
    ) -> eyre::Result<std::convert::Infallible> {
        let status = exec_status_cargo(args_fn)?;
        match status {}
    }
    pub fn exec_status_cargo(
        args_fn: impl FnOnce(&mut Command) -> &mut Command,
    ) -> eyre::Result<std::convert::Infallible> {
        exec_run_cmd(env!("CARGO"), args_fn)
    }
    pub fn exec_run_cmd(
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
                c.args(["--fix", "--allow-dirty"]);
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
    use eyre::Context;
    use std::{
        ffi::OsStr,
        fs::DirEntry,
        path::{Path, PathBuf},
    };

    pub fn run<S>(args: impl IntoIterator<Item = S>) -> eyre::Result<()>
    where
        S: AsRef<OsStr>,
    {
        fn cmd_spigot_visual<S>(
            c: &mut std::process::Command,
            args: impl IntoIterator<Item = S>,
        ) -> &mut std::process::Command
        where
            S: AsRef<OsStr>,
        {
            c.args(["run", "--package", "spigot-visual", "--"])
                //
                .arg("--dev-path-prefix")
                .arg(dist_dir())
                //
                .args(args)
        }

        fmt_js()?;
        dist_js(Some(WriteOutput))?;

        #[cfg(unix)]
        {
            // replace the current process
            crate::unix_exec::exec_run_cargo(|c| cmd_spigot_visual(c, args))
                .map(|never| match never {})
        }

        #[cfg(not(unix))]
        {
            // Fallback for non-Unix systems
            run_cargo(|c| cmd_spigot_visual(c, args))
        }
    }

    pub fn checks(fix: Option<Fix>) -> eyre::Result<()> {
        check_js(fix)?;
        dist_js(None)?;
        Ok(())
    }
    fn check_js(fix: Option<Fix>) -> eyre::Result<()> {
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
        gen_bindings_ts()?;

        run_cmd("tsc", |c| {
            if write.is_none() {
                c.arg("--noEmit");
            }
            c.current_dir(ts_src_dir())
        })
    }

    fn gen_bindings_ts() -> eyre::Result<()> {
        const IGNORE_FILE_NAMES: &[&str] = &[
            // rustfmt hint
            ".gitignore",
        ];
        const KNOWN_BINDING_NAMES: &[&str] = &[
            "Cell",
            "NodeDetails",
            "NodeKind",
            "OrderType",
            "Path",
            "Row",
            "TableView",
        ];
        const EXTENSION_BINDING: &str = "ts";
        const EXTENSION_DEST: &str = "d.ts";

        fn remove_generated_file(entry: &DirEntry, extension: &str) -> eyre::Result<()> {
            let entry_path = entry.path();
            let entry_type = entry.file_type().context("cannot read file type")?;

            // check for type "file" ...
            if !entry_type.is_file() {
                eyre::bail!("expected file, found {entry_type:?}")
            }
            // ... with a filename
            let Some(full_name) = entry_path.file_name() else {
                eyre::bail!("no filename")
            };
            // (skip ignored)
            if IGNORE_FILE_NAMES.iter().any(|v| *v == full_name) {
                return Ok(());
            }

            let Ok(full_name) = String::from_utf8(full_name.as_encoded_bytes().to_vec()) else {
                eyre::bail!("non-utf8 filename: {}", full_name.display())
            };
            let Some((name, extension_multi)) = full_name.split_once('.') else {
                eyre::bail!("no filename extension")
            };

            // ... with the expected extension
            if extension != extension_multi {
                eyre::bail!(
                    "refusing to remove unknown file extension: {}",
                    entry_path.display()
                )
            }
            // ... with any expected name
            if !KNOWN_BINDING_NAMES.contains(&name) {
                eyre::bail!(
                    "refusing to remove unknown file name: {}",
                    entry_path.display()
                )
            }

            // .. then delete
            std::fs::remove_file(entry_path).context("failed to delete the file")?;

            Ok(())
        }
        fn remove_generated_dir(dir: &Path, extension: &str) -> eyre::Result<()> {
            let listing = std::fs::read_dir(dir);
            if let Err(e) = &listing
                && e.kind() == std::io::ErrorKind::NotFound
            {
                return Ok(());
            }
            let listing =
                listing.with_context(|| format!("cannot list folder: {}", dir.display()))?;

            for entry in listing {
                let entry = entry?;
                remove_generated_file(&entry, extension)
                    .with_context(|| format!("cannot remove file {}", entry.path().display()))?;
            }

            // verify files removed
            let listing: Result<Vec<DirEntry>, _> = std::fs::read_dir(dir)?.collect();
            let listing = listing?;
            if listing.len() > IGNORE_FILE_NAMES.len() {
                eyre::bail!("extra files in folder {}: {listing:#?}", dir.display())
            }
            Ok(())
        }

        let binding_dir = bucket_spigot_bindings_dir();
        let dest_dir = ts_src_dir().join("bucket-spigot-bindings");

        // delete old files
        remove_generated_dir(&binding_dir, EXTENSION_BINDING)?;
        remove_generated_dir(&dest_dir, EXTENSION_DEST)?;

        // generate bindings
        run_cargo(|c| {
            c.args(["test", "--features", "ts-rs", "export_bindings"])
                .current_dir(bucket_spigot_dir())
        })?;

        // copy bindings into place
        std::fs::create_dir_all(&dest_dir).with_context(|| {
            format!(
                "failed to create destination folder: {}",
                dest_dir.display()
            )
        })?;
        for binding in KNOWN_BINDING_NAMES {
            let src = binding_dir.join(binding).with_extension(EXTENSION_BINDING);
            let dest = dest_dir.join(binding).with_extension(EXTENSION_DEST);
            std::fs::copy(&src, &dest).with_context(|| {
                format!(
                    "failed to copy SRC {} to DEST {}",
                    src.display(),
                    dest.display()
                )
            })?;
        }

        Ok(())
    }

    fn bucket_spigot_dir() -> PathBuf {
        project_root().join("bucket-spigot")
    }
    fn bucket_spigot_bindings_dir() -> PathBuf {
        project_root().join("bucket-spigot/bindings")
    }

    fn ts_src_dir() -> PathBuf {
        project_root().join("spigot-visual/static-ts")
    }
    fn dist_dir() -> PathBuf {
        project_root().join("spigot-visual/static")
    }
}
