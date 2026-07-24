// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Runs `beet-pusher` and associated frontends

use eyre::Context as _;

use crate::unix_exec::exec_cmd_try_args;

/// Spawns the `beet-pusher-webui` server along with the backend `beet-pusher`
/// with `fake-beet`
pub struct WebUiSpawn {}
impl WebUiSpawn {
    /// Spawns the `beet-pusher-webui` server along with the backend `beet-pusher`
    /// with `fake-beet`
    ///
    /// # Errors
    /// Returns an error if any of the setup or execution fails
    pub fn spawn() -> eyre::Result<()> {
        let dir = tempfile::tempdir()?;
        let dir = dir.path();

        let build_bin = |bin_name| {
            escargot::CargoBuild::new()
                .bin(bin_name)
                .run()
                .with_context(|| format!("failed to build {bin_name}"))
        };

        let beet_pusher = build_bin("beet-pusher")?;
        let beet_pusher_webui = build_bin("beet-pusher-webui")?;
        let beet_pusher_webui_spawn = build_bin("beet-pusher-webui-spawn")?;

        let fake_beet = fake_beet::try_build_bin_once()
            .as_ref()
            .context("failed to build fake-beet")?;

        let fake_beet_config = gen_fake_beet_config(dir)?;

        if true {
            todo!("spawn fake_vlc, using a made-up config file")
        }

        exec_cmd_try_args(
            &beet_pusher_webui_spawn.path().to_string_lossy(),
            |cmd| -> eyre::Result<_> {
                cmd.env("BEET", fake_beet.path())
                    .env("BEET_PUSHER_WEBUI", beet_pusher_webui.path())
                    .env("BEET_PUSHER_BACKEND", beet_pusher.path())
                    .env("FAKE_BEET_CONFIG_FILE", fake_beet_config);

                let mut default_var = |key: &str, value: &str| {
                    if let Err(std::env::VarError::NotPresent) = std::env::var(key) {
                        cmd.env(key, value);
                    }
                };
                default_var("RUST_LOG", "beet_pusher=trace");

                let mut get_or_default_var = |key: &str, value: String| match std::env::var(key) {
                    Ok(value) => Ok(value),
                    Err(std::env::VarError::NotPresent) => {
                        // fill in default value
                        cmd.env(key, &value);
                        Ok(value)
                    }
                    Err(std::env::VarError::NotUnicode(value)) => {
                        eyre::bail!("non-unicode env var {key:?}: {value:?}")
                    }
                };
                let bind_ip = get_or_default_var("BIND_IP", "127.0.0.1".to_string())?;
                let port = get_or_default_var("PORT", "8080".to_string())?;

                eprintln!();
                eprintln!("{0:=<80}", "");
                eprintln!("Navigate to: http://{bind_ip}:{port}/swagger-ui");
                eprintln!("{0:=<80}", "");
                eprintln!();

                Ok(cmd)
            },
        )?
        .map(|never| match never {})
    }
}

fn gen_fake_beet_config(dir: &std::path::Path) -> eyre::Result<std::path::PathBuf> {
    let fake_beet_config = fake_beet::ConfigAll::setup_with(|c| {
        let dummy_long_response = "too many to count I mean so many items it's just almost endless with no filters specified"
        .split_ascii_whitespace()
        .enumerate()
        .map(|(n, word)| format!("{n}={word}"));
        c.for_args(["ls", "-f$id=$path"])
            .stdout_lines(dummy_long_response);

        c.for_args(["args"]).stdout_lines(["line1", "line2"]);
    });

    fake_beet_config
        .create_config_file(dir, "fake-beet-config.txt")
        .context("failed to write fake-beet config file")
}
