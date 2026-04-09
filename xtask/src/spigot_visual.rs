// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Tasks for the `spigot_visual` crate

use self::gen_bindings::gen_bindings_ts;
use crate::{
    Fix, TypedErr, TypedResult, WriteOutput, print_help_fix_checks, project_root, run_cmd,
};
use eyre::Context;
use std::{
    ffi::{OsStr, OsString},
    path::PathBuf,
};

/// Hint that prerequisite files for the `cargo` workspace are present
pub struct HintAllowRustWorkspaceCalls {}
impl HintAllowRustWorkspaceCalls {
    /// Checks that the prerequisite files for the `cargo` workspace are present
    ///
    /// # Errors
    /// Returns an error if reading the file status fails
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
    ///
    /// # Errors
    /// Returns an error if the check or need fails
    pub fn check_and_run_once(fix: Option<Fix>) -> TypedResult<Self> {
        Self::check()?
            .or_else(|need| {
                need.run(fix)?;
                match Self::check()? {
                    Ok(v) => Ok(v),
                    Err(need_next) => {
                        crate::bail!("too many needs: {need:?} --> {need_next:?}")
                    }
                }
            })
            .map_err(|e: TypedErr| {
                e.map_eyre_only(|e| e.context("failed to build files needed for rust workspace"))
            })
    }
}
/// Required action for prerequisite of the cargo workspace
pub enum Need {
    /// Need to run [`DistJs::dist_js()`]
    DistJsWrite {
        /// Prerequisite files that are missing
        missing_files: Vec<PathBuf>,
    },
}
impl Need {
    /// Attempts to resolve the prerequisite
    ///
    /// # Errors
    /// Returns an error if `fix` is not specified, or the file generation fails
    pub fn run(&self, fix: Option<Fix>) -> TypedResult<()> {
        let Some(Fix::Fix) = fix else {
            crate::bail!("argument `fix` not specified, refusing to write output files: {self:#?}")
        };
        println!(
            "building source prerequisite for rust workpace calls:\n\t{:?}\n",
            self.label()
        );
        match self {
            Need::DistJsWrite {
                missing_files: _diagnostic_only,
            } => DistJs::default().dist_js(),
        }
    }
    fn label(&self) -> &'static str {
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

/// Runs spigot-visual with the compiled typescript
#[derive(Debug, clap::Args)]
pub struct Run {
    args: Vec<OsString>,
}
impl Run {
    /// Generates prerequisit files and executes the `spigot-visual` main entrypoint
    ///
    /// # Errors
    /// Returns any errors from I/O or spawned subprocesses
    pub fn run(self) -> TypedResult<()> {
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

        let Self { args } = self;

        fmt_js()?;

        DistJs::default().dist_js()?;

        #[cfg(unix)]
        {
            // replace the current process
            let never = crate::unix_exec::exec_cargo(|c| cmd_spigot_visual(c, args))?;
            match never {}
        }

        #[cfg(not(unix))]
        {
            // Fallback for non-Unix systems
            run_cargo(|c| cmd_spigot_visual(c, args))
        }
    }
}

/// Runs all checks for `spigot-visual`
///
/// # Errors
/// Returns any fatal errors with the checks
pub fn checks(fix: Option<Fix>) -> TypedResult<()> {
    check_js(fix)?;
    DistJs { write: None }.dist_js()?;
    Ok(())
}
fn check_js(fix: Option<Fix>) -> TypedResult<()> {
    run_cmd("biome", |c| {
        c.arg("check");
        if let Some(Fix::Fix) = fix {
            c.arg("--write");
        }
        c.current_dir(ts_src_dir())
    })
    .inspect_err(|e| print_help_fix_checks(e, &fix))
}

/// Formats the JavaScript sources
fn fmt_js() -> TypedResult<()> {
    // `biome format --write` also works, but format is a subset of `biome check --write`
    check_js(Some(Fix::Fix))
}

/// Compiles the spigot-visual typescript
#[derive(Clone, Copy, Debug, clap::Args)]
pub struct DistJs {
    #[clap(subcommand)]
    write: Option<WriteOutput>,
}
impl Default for DistJs {
    fn default() -> Self {
        Self {
            write: Some(WriteOutput::Write),
        }
    }
}
impl DistJs {
    /// Generates the JavaScript sources
    ///
    /// # Errors
    /// Returns an error if any subprocesses fail
    pub fn dist_js(self) -> TypedResult<()> {
        let Self { write } = self;

        gen_bindings_ts()?;

        run_cmd("tsc", |c| {
            if write.is_none() {
                c.arg("--noEmit");
            }
            c.current_dir(ts_src_dir())
        })
    }
}

mod gen_bindings {
    use eyre::Context as _;
    use std::{
        fs::DirEntry,
        path::{Path, PathBuf},
    };

    use self::ts_binding_set::TsBindingSet;
    use crate::TypedResult;

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

    pub(super) fn gen_bindings_ts() -> TypedResult<()> {
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
        use crate::{TypedResult, project_root, run_cargo, spigot_visual::ts_src_dir};
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
            pub fn execute(self) -> TypedResult<()> {
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
        let listing = listing.with_context(|| format!("cannot list folder: {}", dir.display()))?;

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
