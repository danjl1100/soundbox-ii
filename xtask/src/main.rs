//! Helper commands for the repo, following the
//! [`cargo-xtask`](https://github.com/matklad/cargo-xtask/) template

use std::path::{Path, PathBuf};

fn main() -> eyre::Result<()> {
    let mut args = std::env::args().skip(1);
    let task = args.next();
    match task.as_deref() {
        Some("spigot-visual-run") => spigot_visual::run(args)?,
        Some("spigot-visual-dist") => spigot_visual::dist()?,
        _ => print_help(),
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

    use crate::project_root;
    use eyre::Context as _;
    use std::{ffi::OsStr, path::PathBuf, process::Command};

    pub fn run<S>(args: impl IntoIterator<Item = S>) -> eyre::Result<()>
    where
        S: AsRef<OsStr>,
    {
        dist()?;

        let status = Command::new(env!("CARGO"))
            .args(["run", "--package", "spigot-visual", "--"])
            //
            .arg("--dev-path-prefix")
            .arg(dist_dir())
            //
            .args(args)
            .status()
            .context("failed to run `cargo`")?;

        if !status.success() {
            eyre::bail!("command failed");
        }
        Ok(())
    }

    pub fn dist() -> eyre::Result<()> {
        let status = Command::new("tsc")
            .current_dir(ts_src_dir())
            .status()
            .context("failed to run `tsc`")?;

        if !status.success() {
            eyre::bail!("`tsc` failed");
        }

        Ok(())
    }

    fn ts_src_dir() -> PathBuf {
        project_root().join("spigot-visual/static-ts")
    }
    fn dist_dir() -> PathBuf {
        project_root().join("spigot-visual/static")
    }
}
