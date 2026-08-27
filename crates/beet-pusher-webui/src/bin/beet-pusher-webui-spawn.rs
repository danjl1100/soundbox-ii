// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Helper utility to spawn both `beet_pusher_webui` and `beet_pusher` with stdio
//! pipes connected appropriately

fn main() -> eyre::Result<()> {
    let args = self::spawner::Args::try_new()?;
    args.run()
}

mod spawner {
    //! Functions to run `beet_pusher_webui` and `beet_pusher` with stdio pipes
    //!
    //! NOTE: This functionality is included as bin in this crate
    // (and not just in the the internal `xtasks` crate)
    //! as the final binary consumer will likely need to recreate this
    //! exact setup.

    use std::{process::Stdio, thread::JoinHandle};

    use eyre::{Context as _, OptionExt as _};

    #[derive(Debug)]
    pub struct Args {
        webui_env: std::path::PathBuf,
        backend_env: std::path::PathBuf,
    }

    const ENV_BEET_PUSHER_WEBUI: &str = "BEET_PUSHER_WEBUI";
    const ENV_BEET_PUSHER_BACKEND: &str = "BEET_PUSHER_BACKEND";

    impl Args {
        /// Returns the spawner arguments if the spawner's main should run
        pub fn try_new() -> eyre::Result<Self> {
            let Ok(webui_env) = std::env::var(ENV_BEET_PUSHER_WEBUI) else {
                eyre::bail!("{ENV_BEET_PUSHER_WEBUI} env string not specified");
            };
            let Ok(backend_env) = std::env::var(ENV_BEET_PUSHER_BACKEND) else {
                eyre::bail!("{ENV_BEET_PUSHER_BACKEND} env string not specified");
            };
            Ok(Self {
                webui_env: std::path::PathBuf::from(webui_env),
                backend_env: std::path::PathBuf::from(backend_env),
            })
        }
    }

    impl Args {
        pub fn run(self) -> eyre::Result<()> {
            let Self {
                webui_env,
                backend_env,
            } = self;

            let mut cmd_backend = std::process::Command::new(backend_env)
                .arg("--json")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()?;

            let mut cmd_webui = std::process::Command::new(&webui_env)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()
                .with_context(|| format!("failed to launch webui {}", webui_env.display()))?;

            let t1 = spawn_copy(cmd_webui.stdout.take(), cmd_backend.stdin.take())?;
            let t2 = spawn_copy(cmd_backend.stdout.take(), cmd_webui.stdin.take())?;
            let result1 = t1.join().expect("panic in spawn_copy thread");
            let result2 = t2.join().expect("panic in spawn_copy thread");
            match (result1, result2) {
                (Ok(()), Ok(())) => Ok(()),
                (Ok(()), Err(e)) | (Err(e), Ok(())) => Err(e).context("stdio stream copy failed"),
                (Err(e1), Err(e2)) => {
                    eyre::bail!("stdio stream copy errors: {e1} and {e2}");
                }
            }
        }
    }

    fn spawn_copy(
        reader: Option<std::process::ChildStdout>,
        writer: Option<std::process::ChildStdin>,
    ) -> eyre::Result<JoinHandle<std::io::Result<()>>> {
        let reader = reader.ok_or_eyre("reader missing")?;
        let writer = writer.ok_or_eyre("writer missing")?;

        let thread_handle = std::thread::spawn(move || {
            let mut reader = std::io::BufReader::new(reader);
            let mut writer = std::io::BufWriter::new(writer);

            std::io::copy(&mut reader, &mut writer).map(|_len| ())
        });
        Ok(thread_handle)
    }
}
