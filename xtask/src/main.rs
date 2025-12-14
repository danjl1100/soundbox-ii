// Copyright (C) 2021-2025  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Helper commands for the repo, following the
//! [`cargo-xtask`](https://github.com/matklad/cargo-xtask/) template

use crate::spigot_visual::HintAllowRustWorkspaceCalls;
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

    let hint = HintAllowRustWorkspaceCalls::check_and_run_once(fix)?;

    rust::checks(fix, &hint)?;
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
    use crate::{Fix, run_cargo, spigot_visual::HintAllowRustWorkspaceCalls, status_cargo};

    pub fn checks(fix: Option<Fix>, _hint: &HintAllowRustWorkspaceCalls) -> eyre::Result<()> {
        fmt(fix)?;
        clippy(fix)?;
        test()?;
        doc()?;

        Ok(())
    }

    fn fmt(fix: Option<Fix>) -> eyre::Result<()> {
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

    use self::gen_bindings::gen_bindings_ts;
    use crate::{Fix, WriteOutput, print_help_fix_checks, project_root, run_cmd};
    use eyre::Context;
    use std::{ffi::OsStr, path::PathBuf};

    pub struct HintAllowRustWorkspaceCalls {}
    impl HintAllowRustWorkspaceCalls {
        pub fn check() -> eyre::Result<Result<Self, Need>> {
            /// Generated files that are required for workspace-wide cargo invocations
            const REQUIRED_DIST_DIR_FILES: &[&str] = &["app.js", "sample-input.js"];
            let missing_files = REQUIRED_DIST_DIR_FILES
                .iter()
                .filter_map(|name| {
                    let path = {
                        let mut p = dist_dir();
                        p.push(name);
                        p
                    };
                    std::fs::exists(&path)
                        .with_context(|| format!("failed to stat {}", path.display()))
                        .map(|exists| (!exists).then_some(path))
                        .transpose()
                })
                .collect::<Result<Vec<_>, _>>()?;
            if !missing_files.is_empty() {
                return Ok(Err(Need::DistJsWrite { missing_files }));
            }
            Ok(Ok(Self {}))
        }
        /// Automatically attempts to recover if [`Self::check()`] returns a [`Need`] (one time only)
        pub fn check_and_run_once(fix: Option<Fix>) -> eyre::Result<Self> {
            Self::check()?
                .or_else(|need| {
                    need.run(fix)?;
                    match Self::check()? {
                        Ok(v) => Ok(v),
                        Err(need_next) => {
                            eyre::bail!("too many needs: {need:?} --> {need_next:?}");
                        }
                    }
                })
                .context("failed to build files needed for rust workspace")
        }
    }
    pub enum Need {
        /// Need to run [`dist_js()`]
        DistJsWrite { missing_files: Vec<PathBuf> },
    }
    impl Need {
        pub fn run(&self, fix: Option<Fix>) -> eyre::Result<()> {
            let Some(Fix) = fix else {
                eyre::bail!(
                    "argument `fix` not specified, refusing to write output files: {self:#?}"
                )
            };
            println!(
                "building source dependency for rust workpace calls:\n\t{:?}\n",
                self.label()
            );
            match self {
                Need::DistJsWrite {
                    missing_files: _diagnostic_only,
                } => dist_js(Some(WriteOutput)),
            }
        }
        pub fn label(&self) -> &'static str {
            match self {
                Need::DistJsWrite { .. } => "cargo xtask spigot-visual-dist",
            }
        }
    }
    impl std::fmt::Debug for Need {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let mut debug = f.debug_struct("");

            match self {
                Need::DistJsWrite { missing_files } => {
                    debug.field("missing_files", missing_files);
                }
            }

            debug.field("add `fix` argument to automatically run", &self.label());
            debug.finish()
        }
    }

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

    mod gen_bindings {
        use eyre::Context as _;
        use std::{
            fs::DirEntry,
            path::{Path, PathBuf},
        };

        use self::ts_binding_set::TsBindingSet;

        /// files that are skipped for auto-removal (without any warnings)
        const IGNORE_FILE_NAMES: &[&str] = &[
            // rustfmt hint
            ".gitignore",
            // manual listing of all kinds
            // TODO make this automatic, this module knows the binding names!
            "index.d.ts",
        ];
        const EXTENSION_BINDING: &str = "ts";
        const EXTENSION_DEST: &str = "d.ts";

        pub(super) fn gen_bindings_ts() -> eyre::Result<()> {
            TsBindingSet::new()
                .add_package(
                    "bucket-spigot",
                    &[
                        "Cell",
                        "NodeDetails",
                        "NodeKind",
                        "OrderType",
                        "Path",
                        "Row",
                        "TableView",
                    ],
                )
                .add_package(
                    "spigot-visual",
                    &[
                        "NetworkModifyCmd",
                        "OrderType",
                        "Path",
                        "SpigotCommandKind",
                        "SpigotResponse",
                    ],
                )
                .execute()
        }

        mod ts_binding_set {
            use super::{EXTENSION_DEST, copy_bindings_to_dest, remove_generated_dir};
            use crate::{project_root, run_cargo, spigot_visual::ts_src_dir};
            use std::path::PathBuf;

            #[derive(Default)]
            pub(super) struct TsBindingSet {
                package_names: Vec<&'static str>,
                packages: Vec<Package>,
            }
            impl TsBindingSet {
                pub fn new() -> Self {
                    Self::default()
                }
                pub fn add_package(
                    mut self,
                    package_name: &'static str,
                    known_binding_names: &'static [&'static str],
                ) -> Self {
                    let Self {
                        package_names,
                        packages,
                    } = &mut self;
                    package_names.push(package_name);

                    packages.push(Package {
                        binding_dir: project_root().join(format!("{package_name}/bindings")),
                        dest_dir: ts_src_dir().join(format!("{package_name}-bindings")),
                        known_binding_names,
                    });

                    self
                }
                pub fn execute(self) -> eyre::Result<()> {
                    let Self {
                        package_names,
                        packages,
                    } = self;

                    // remove old generated (source) files
                    for pkg in &packages {
                        pkg.remove_generated_dir()?;
                    }

                    // TODO - do more in each package?
                    // // delete old generated files (destination) files
                    // remove_generated_dir(&binding_dir, EXTENSION_BINDING, known_binding_names)?;

                    // generate bindings
                    run_cargo(|c| {
                        c.args([
                            "test",
                            // some crates may gate `ts-rs` on a feature flag
                            "--features",
                            "ts-rs",
                            // run the "export_bindings" test modules, generated by `ts-rs`
                            "export_bindings",
                            // ignore "unit tests" in binary targets
                            "--lib",
                            // print '.' for each test, instead of a full line
                            "--quiet",
                        ]);
                        for package in package_names {
                            c.arg("--package").arg(package);
                        }
                        c.current_dir(project_root());
                        c
                    })?;

                    // copy new generated files into destination
                    for pkg in packages {
                        pkg.copy_bindings_to_dest()?;
                    }

                    Ok(())
                }
            }

            struct Package {
                binding_dir: PathBuf,
                dest_dir: PathBuf,
                known_binding_names: &'static [&'static str],
            }
            impl Package {
                fn remove_generated_dir(&self) -> eyre::Result<()> {
                    let Self {
                        binding_dir: _, // TODO unused? or remove files there too?
                        dest_dir,
                        known_binding_names,
                    } = self;
                    // delete old files
                    remove_generated_dir(dest_dir, EXTENSION_DEST, known_binding_names)
                }
                fn copy_bindings_to_dest(self) -> eyre::Result<()> {
                    use std::fmt::Write as _;

                    let Self {
                        binding_dir,
                        dest_dir,
                        known_binding_names,
                    } = self;

                    // copy bindings into place
                    copy_bindings_to_dest(known_binding_names, &binding_dir, &dest_dir)?;

                    // generate an import helper
                    let index = {
                        let mut p = dest_dir;
                        p.push("index.d.ts");
                        p
                    };
                    let mut index_contents =
                        "// Automatically generated by soundbox-iii xtask\n\n".to_string();
                    for name in known_binding_names {
                        writeln!(
                            &mut index_contents,
                            "export {{ {name} }} from './{name}.d.ts';"
                        )
                        .expect("infallible");
                    }
                    std::fs::write(index, index_contents)?;

                    Ok(())
                }
            }
        }

        fn copy_bindings_to_dest(
            names: &[&str],
            binding_dir: &Path,
            dest_dir: &Path,
        ) -> eyre::Result<()> {
            std::fs::create_dir_all(dest_dir).with_context(|| {
                format!(
                    "failed to create destination folder: {}",
                    dest_dir.display()
                )
            })?;
            for name in names {
                let src = binding_dir.join(name).with_extension(EXTENSION_BINDING);
                let dest = dest_dir.join(name).with_extension(EXTENSION_DEST);
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

        fn remove_generated_dir(
            dir: &Path,
            extension: &str,
            known_binding_names: &[&str],
        ) -> eyre::Result<()> {
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
                remove_generated_file(&entry, extension, known_binding_names)
                    .with_context(|| format!("cannot remove file {}", entry.path().display()))?;
            }

            // verify files removed
            let extra_files = std::fs::read_dir(dir)?
                .filter_map(|entry| {
                    (|| {
                        let entry = entry?;
                        let path = entry.path();
                        let name = entry.file_name();
                        if let Some(name) = name.to_str()
                            && IGNORE_FILE_NAMES.contains(&name)
                        {
                            Ok(None)
                        } else {
                            Ok(Some(path))
                        }
                    })()
                    .transpose()
                })
                .collect::<Result<Vec<PathBuf>, std::io::Error>>()?;
            if !extra_files.is_empty() {
                eyre::bail!("extra files in folder {}: {extra_files:#?}", dir.display())
            }
            Ok(())
        }
        fn remove_generated_file(
            entry: &DirEntry,
            extension: &str,
            known_binding_names: &[&str],
        ) -> eyre::Result<()> {
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
            let (name, extension_multi) = full_name
                .split_once('.')
                .map_or((&*full_name, None), |(n, ext)| (n, Some(ext)));

            // ... with any expected name
            if !known_binding_names.contains(&name) {
                eyre::bail!(
                    "refusing to remove unknown file name: {}",
                    entry_path.display()
                )
            }
            // ... with the expected extension
            if Some(extension) != extension_multi {
                eyre::bail!(
                    "refusing to remove unknown file extension: {}",
                    entry_path.display()
                )
            }

            // .. then delete
            std::fs::remove_file(entry_path).context("failed to delete the file")?;

            Ok(())
        }
    }

    fn ts_src_dir() -> PathBuf {
        project_root().join("spigot-visual/static-ts")
    }
    fn dist_dir() -> PathBuf {
        project_root().join("spigot-visual/static")
    }
}
