// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! HTTP server frontend to drive [`beet_pusher`]

use beet_pusher_webui::{
    config::Config,
    create_app,
    infra::{Shutdown, stdio_pipe::StdioPipe},
    init_tracing,
};
use eyre::Context as _;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    if let Some(args) = self::spawner::Args::try_new() {
        // run helper utility to spawn both beet_pusher_webui and beet_pusher
        // with stdio pipes connected appropriately
        let args = args?;
        let task = tokio::task::spawn_blocking(|| args.run());
        return task.await?;
    }

    init_tracing("INFO");

    tracing::info!("Startup");

    let config = Config::from_env()?;

    let (shutdown_tx, shutdown_rx) = tokio::sync::mpsc::channel(1);

    let (pipe, stdout_thread, stdin_thread) = StdioPipe::spawn(shutdown_tx);
    let app = create_app(config.clone(), pipe).await;

    let addr = format!("{}:{}", config.bind_ip, config.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .with_context(|| format!("failed to bind TCP listener to {addr}"))?;

    let addr = listener.local_addr()?;
    tracing::info!(%addr, "Listening for HTTP connections");

    if let Ok(port_path) = std::env::var("SCRIPT_WRITE_PORT") {
        std::thread::spawn(move || {
            let contents = format!("{}", addr.port());
            let result = std::fs::write(port_path, &contents);

            if let Err(e) = result {
                let err = eyre::eyre!(e);
                tracing::error!(?err, "failed to write port file");
            }
        });
    }

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(shutdown_rx))
        .await?;

    stdout_thread.join().expect("panic in pipe thread")?;
    stdin_thread.join().expect("panic in pipe thread")?;

    Ok(())
}

async fn shutdown_signal(mut shutdown_rx: tokio::sync::mpsc::Receiver<Shutdown>) {
    shutdown_rx.recv().await;
}

mod spawner {
    //! Functions to run `beet_pusher_webui` and `beet_pusher` with stdio pipes
    //!
    //! NOTE: This functionality is included as part of this crate (and not
    //! xtasks) as the final binary consumer will likely need to recreate this
    //! exact setup.

    use std::{process::Stdio, thread::JoinHandle};

    use eyre::{Context as _, OptionExt as _};

    #[derive(Debug)]
    pub struct Args {
        webui_arg_0: std::path::PathBuf,
        backend_env: std::path::PathBuf,
    }

    const ENV_BEET_PUSHER_BACKEND: &str = "BEET_PUSHER_BACKEND";

    const ENV_BEET_PUSHER_BLOCK_SPAWNER: &str = "BEET_PUSHER_BLOCK_SPAWNER";
    const ENV_BEET_PUSHER_BLOCK_SPAWNER_VALUE: &str = "1";

    impl Args {
        /// Returns the spawner arguments if the spawner's main should run
        pub fn try_new() -> Option<eyre::Result<Self>> {
            if !is_env(
                ENV_BEET_PUSHER_BLOCK_SPAWNER,
                ENV_BEET_PUSHER_BLOCK_SPAWNER_VALUE,
            ) && let Some(webui_arg_0) = is_bin_name("beet-pusher-webui-spawn")
            {
                let result = if let Ok(backend_env) = std::env::var(ENV_BEET_PUSHER_BACKEND) {
                    Ok(Self {
                        webui_arg_0,
                        backend_env: std::path::PathBuf::from(backend_env),
                    })
                } else {
                    Err(eyre::eyre!("{ENV_BEET_PUSHER_BACKEND} not specified"))
                };
                Some(result)
            } else {
                None
            }
        }
    }

    /// Returns the arg 0 string if the filename matches the specified name
    fn is_bin_name(name: &str) -> Option<std::path::PathBuf> {
        let arg_0 = std::env::args().next()?;

        let arg_0 = std::path::PathBuf::from(arg_0);
        if let Some(bin_name) = arg_0.file_name()
            && bin_name == name
        {
            Some(arg_0)
        } else {
            None
        }
    }

    fn is_env(key: &str, value: &str) -> bool {
        let Ok(var) = std::env::var(key) else {
            return false;
        };
        var == value
    }

    impl Args {
        pub fn run(self) -> eyre::Result<()> {
            let Self {
                webui_arg_0,
                backend_env,
            } = self;

            let mut cmd_backend = std::process::Command::new(backend_env)
                .arg("--json")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()?;

            let mut cmd_webui = std::process::Command::new(webui_arg_0)
                .env(
                    ENV_BEET_PUSHER_BLOCK_SPAWNER,
                    ENV_BEET_PUSHER_BLOCK_SPAWNER_VALUE,
                )
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()?;

            let t1 = spawn_copy(cmd_webui.stdout.take(), cmd_backend.stdin.take())?;
            let t2 = spawn_copy(cmd_backend.stdout.take(), cmd_webui.stdin.take())?;
            let result1 = t1.join().expect("panic in spawn_copy thread");
            let result2 = t2.join().expect("panic in spawn_copy thread");
            match (result1, result2) {
                (Ok(()), Ok(())) => Ok(()),
                (Ok(()), Err(e)) | (Err(e), Ok(())) => Err(e).context("stdio stream copy failed"),
                (Err(e1), Err(e2)) => eyre::bail!("stdio stream copy errors: {e1} and {e2}"),
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
