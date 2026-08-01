// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Runs `beet-pusher` and associated frontends

use eyre::Context as _;

use crate::{CmdSettings, TypedResult, status_cmd::spawn_cmd_try_args};

/// Spawns the `beet-pusher-webui` server along with the backend `beet-pusher`
/// with `fake-beet`
#[derive(Clone, Debug, clap::Args)]
pub struct WebUiSpawn {
    #[clap(flatten)]
    cmd_args: crate::ArgsCmdSettings,
    #[clap(flatten)]
    args: ArgsInput,
}
#[derive(Clone, Debug, clap::Args)]
struct ArgsInput {
    #[clap(long)]
    simulate_beet: bool,
    #[clap(long)]
    simulate_vlc: bool,
    #[clap(long)]
    simulate_all: bool,
}

impl WebUiSpawn {
    /// Spawns the `beet-pusher-webui` server along with the backend `beet-pusher`
    /// with `fake-beet`
    ///
    /// # Errors
    /// Returns an error if any of the setup or execution fails
    pub fn spawn(self) -> TypedResult<()> {
        let Self { cmd_args, args } = self;

        let cmd_settings = cmd_args.into_inner();

        let Binaries {
            beet_pusher,
            beet_pusher_webui,
            beet_pusher_webui_spawn,
        } = Binaries::build(&cmd_settings)?;

        let dir = tempfile::tempdir().context("failed to create tempdir for config files")?;
        let dir = dir.path();

        let args = ArgsCombined::from(args);

        // print help messages
        eprintln!("{args}");

        let ArgsCombined {
            simulate_vlc,
            simulate_beet,
        } = args;

        let fake_beet_configs = simulate_beet
            .then(|| -> eyre::Result<_> {
                let fake_beet = fake_beet::try_build_bin_once()
                    .as_ref()
                    .context("failed to build fake-beet")?;

                let config = gen_fake_beet_config(dir)?;
                eprintln!("Created fake-beet config: {}", config.display());

                // `beet-pusher` reads the `beet` executable path from its own config file, so it
                // has to be generated here to point at the `fake-beet` build
                let pusher_config = gen_beet_pusher_config(dir, fake_beet.path())?;
                eprintln!("Created beet-pusher config: {}", pusher_config.display());

                Ok((config, pusher_config))
            })
            .transpose()?;

        let (vlc_auth_file, fake_vlc_and_thread) = simulate_vlc
            .then(|| -> eyre::Result<_> {
                let (fake_vlc, thread) = ::fake_vlc::FakeVlc::new()?;
                eprintln!("Spawned fake-vlc");

                let vlc_auth_file = fake_vlc.create_config_file(dir, "fake-vlc.toml")?;
                eprintln!("Created fake-vlc config: {}", vlc_auth_file.display());

                Ok((vlc_auth_file, (fake_vlc, thread)))
            })
            .transpose()?
            .unzip();

        // Spawn (not wait) on the beet-pusher-webui-spawn process to allow
        // fake-vlc to (interactively?) run in this process
        // NOTE: Must wait on the child to finish before tearing down fake-vlc
        // and the tempdir
        let mut beet_pusher_webui_spawn = spawn_cmd_try_args(
            &beet_pusher_webui_spawn.path().to_string_lossy(),
            |cmd| -> eyre::Result<_> {
                cmd.env("BEET_PUSHER_WEBUI", beet_pusher_webui.path())
                    .env("BEET_PUSHER_BACKEND", beet_pusher.path());

                if let Some((fake_beet_config, pusher_config)) = &fake_beet_configs {
                    cmd.env("FAKE_BEET_CONFIG_FILE", fake_beet_config)
                        .env("BEET_PUSHER_CONFIG_FILE", pusher_config);
                }

                if let Some(vlc_auth_file) = &vlc_auth_file {
                    cmd.env("VLC_AUTH_FILE", vlc_auth_file);
                }

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
                let bind_ip =
                    get_or_default_var("BEET_PUSHER_WEBUI__BIND_IP", "127.0.0.1".to_string())?;
                let port = get_or_default_var("BEET_PUSHER_WEBUI__PORT", "8080".to_string())?;

                eprintln!();
                eprintln!("{0:=<80}", "");
                eprintln!("Navigate to: http://{bind_ip}:{port}/swagger-ui");
                eprintln!("{0:=<80}", "");
                eprintln!();

                Ok(cmd)
            },
        )??;

        // interactive for the user
        let mut kill_requested = false;
        for line in std::io::stdin().lines() {
            let line = line.context("failed to read stdin")?;
            let line = line.trim();

            if line.is_empty() {
                continue;
            }

            match line {
                "q" | "quit" => {
                    beet_pusher_webui_spawn
                        .kill()
                        .context("failed to terminate beet-pusher-webui-spawn")?;

                    kill_requested = true;
                    break;
                }
                _ => {
                    eprintln!(
                        "unknown: {line:?}
q | quit = kills the processes, to cleanup the harness tempdir correctly"
                    );
                }
            }
        }

        let status = beet_pusher_webui_spawn
            .wait()
            .context("failed to wait for beet-pusher-webui-spawn")?;

        #[allow(
            clippy::missing_panics_doc,
            reason = "passthru panic from spawned thread"
        )]
        if let Some((fake_vlc, thread)) = fake_vlc_and_thread {
            drop(fake_vlc);
            thread
                .join()
                .expect("fake_vlc thread panic")
                .context("fake_vlc thread failed")?;
        }

        if !status.success() {
            if kill_requested {
                eprintln!("beet-pusher-webui-spawn killed ({status})");
            } else {
                return Err(eyre::eyre!("beet-pusher-webui-spawn failed: {status}").into());
            }
        }

        Ok(())
    }
}

#[expect(clippy::struct_field_names, reason = "bin names have a common prefix")]
struct Binaries {
    beet_pusher: escargot::CargoRun,
    beet_pusher_webui: escargot::CargoRun,
    beet_pusher_webui_spawn: escargot::CargoRun,
}
impl Binaries {
    fn build(cmd_settings: &CmdSettings) -> TypedResult<Self> {
        // `cargo build` all first, for consolidated error messages
        cmd_settings.run_cargo(|c| {
            c.args([
                "build",
                "--bin",
                "beet-pusher",
                "--bin",
                "beet-pusher-webui",
                "--bin",
                "beet-pusher-webui-spawn",
            ])
        })?;

        let build_bin = |bin_name| {
            escargot::CargoBuild::new()
                .bin(bin_name)
                .run()
                .with_context(|| format!("failed to build {bin_name}"))
        };

        // build each again (should be cached), for easy path discovery
        let beet_pusher = build_bin("beet-pusher")?;
        let beet_pusher_webui = build_bin("beet-pusher-webui")?;
        let beet_pusher_webui_spawn = build_bin("beet-pusher-webui-spawn")?;

        Ok(Self {
            beet_pusher,
            beet_pusher_webui,
            beet_pusher_webui_spawn,
        })
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

fn gen_beet_pusher_config(
    dir: &std::path::Path,
    fake_beet: &std::path::Path,
) -> eyre::Result<std::path::PathBuf> {
    fake_beet::create_config_file(
        dir,
        "beet-pusher.config.toml",
        &format!(
            r#"
            base_url = "file:///fake-beet-media/"
            beet = {beet:?}
            "#,
            beet = fake_beet.to_string_lossy(),
        ),
    )
    .context("failed to write beet-pusher config file")
}

/// Simplified functional flags, displayed as help text
struct ArgsCombined {
    simulate_vlc: bool,
    simulate_beet: bool,
}
impl From<ArgsInput> for ArgsCombined {
    fn from(value: ArgsInput) -> Self {
        let ArgsInput {
            mut simulate_beet,
            mut simulate_vlc,
            simulate_all,
        } = value;
        simulate_vlc |= simulate_all;
        simulate_beet |= simulate_all;
        Self {
            simulate_vlc,
            simulate_beet,
        }
    }
}
impl std::fmt::Display for ArgsCombined {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            simulate_vlc,
            simulate_beet,
        } = *self;

        let hint_line = |f: &mut std::fmt::Formatter<'_>, noun, action| {
            write!(f, "\n  HINT: Use {noun} to {action}")
        };

        if simulate_vlc {
            write!(f, "- simulating VLC")?;
        } else {
            write!(f, "- using external VLC with provided config")?;
            hint_line(f, "VLC_AUTH_FILE", "load a VLC config TOML file")?;
            hint_line(
                f,
                "--simulate-vlc",
                "run without an actual connection to VLC",
            )?;
        }

        writeln!(f)?;

        if simulate_beet {
            write!(f, "- simulating beet")?;
        } else {
            write!(f, "- using external beet executable")?;
            hint_line(
                f,
                "BEET_PUSHER_CONFIG_FILE",
                "load a beet-pusher config TOML file (which sets the beet executable path)",
            )?;
            hint_line(
                f,
                "--simulate-beet",
                "run with a simulated beet data source",
            )?;
        }

        Ok(())
    }
}
