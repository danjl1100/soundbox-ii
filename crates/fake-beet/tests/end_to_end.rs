// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Verifies `fake-beet` performs the actions specified in the configuration

use std::process::Command;

use eyre::Context as _;
use fake_beet::{ConfigAll, ConfigOutput};

#[test]
fn output_reacts_args() -> eyre::Result<()> {
    let mut configs = fake_beet::ConfigAll::default();

    configs
        .for_args(["arg", "set", "123"])
        .stdout("this is the stdout!");

    configs
        .for_args(["another", "set"])
        .stdout("but this stdout is different...");

    configs.for_args_empty([]).stdout("empty args, too");

    verify_all_outputs(configs)?;

    Ok(())
}

#[test]
fn error_reacts_args() -> eyre::Result<()> {
    // verify stdout and stderr
    {
        let mut configs = fake_beet::ConfigAll::default();

        configs
            .for_args(["some invalid", "args"])
            .stdout("start of output...")
            .stderr("then the error output");

        configs.for_args(["other", "one"]).stderr("error info here");

        verify_all_outputs(configs)?;
    }
    // verify status code (specific numbers)
    {
        let mut configs = fake_beet::ConfigAll::default();

        configs
            .for_args(["this", "code"])
            .stdout("some output")
            .stderr("errors")
            .exit_code(23);

        configs.for_args(["another", "code"]).exit_code(228);

        verify_all_outputs(configs)?;
    }

    Ok(())
}

#[test]
fn delay_reacts_args() -> eyre::Result<()> {
    // verify delay
    let mut configs = fake_beet::ConfigAll::default();

    configs
        .for_args(["first", "args"])
        .stdout("start of output...")
        .stderr("then the error output")
        .delay_millis(200);

    configs.for_args(["other", "args"]).delay_millis(2);

    verify_all_outputs(configs)?;

    Ok(())
}

#[test]
fn stdout_multiline() {
    let config1 = {
        let mut c = ConfigAll::default();
        c.for_args_empty([])
            .stdout_lines(["line1", "line2", "line3"]);
        c
    };
    let config2 = {
        let mut c = ConfigAll::default();
        c.for_args_empty([]).stdout("line1\nline2\nline3");
        c
    };

    assert_eq!(config1, config2);
}
#[test]
fn stderr_multiline() {
    let config1 = {
        let mut c = ConfigAll::default();
        c.for_args_empty([])
            .stderr_lines(["line1", "line2", "line3"]);
        c
    };
    let config2 = {
        let mut c = ConfigAll::default();
        c.for_args_empty([]).stderr("line1\nline2\nline3");
        c
    };

    assert_eq!(config1, config2);
}

#[track_caller]
fn verify_all_outputs(config: ConfigAll) -> eyre::Result<()> {
    // setup config file (shared by all invocations)
    let temp_dir = tempfile::tempdir().context("failed to create tempdir")?;
    let dir = temp_dir.path();

    let config_str = &serde_json::to_string_pretty(&config).context("failed to serialize")?;
    let config_file = create_config_file(dir, "fake-beet_config.json", config_str)
        .context("failed to write fake-beet config file")?;

    for (args, expected_out) in config.into_configs_map() {
        let start = std::time::Instant::now();

        let cmd_out = Command::new(fake_beet::build_bin_once().path())
            .args(&args)
            .env(fake_beet::FAKE_BEET_CONFIG_FILE, &config_file)
            .current_dir(dir)
            .output()
            .context("failed to run fake beet")?;

        let elapsed = start.elapsed();

        let ConfigOutput {
            stdout,
            stderr,
            exit_code,
            delay_millis,
        } = expected_out.into_output();
        let exit_code = Some(exit_code.into());

        let cmd_stdout = String::from_utf8_lossy(&cmd_out.stdout);
        let cmd_stderr = String::from_utf8_lossy(&cmd_out.stderr);

        assert_eq!(&*cmd_stderr, stderr, "stderr for fake_beet args {args:?}");
        assert_eq!(&*cmd_stdout, stdout, "stdout for fake_beet args {args:?}");
        assert_eq!(
            cmd_out.status.code(),
            exit_code,
            "exit_code for fake_beet args {args:?}"
        );

        let elapsed_millis = elapsed.as_millis();
        assert!(
            elapsed_millis >= delay_millis.into(),
            "expected {delay_millis}ms delay, but only took {elapsed:?}"
        );
    }

    Ok(())
}

fn create_config_file(
    dir: &std::path::Path,
    file_name: &str,
    file_content: &str,
) -> eyre::Result<std::path::PathBuf> {
    let mut p = dir.to_path_buf();
    p.push(file_name);
    std::fs::write(&p, file_content)
        .with_context(|| format!("failed to create {file_name} at {}", p.display()))?;

    Ok(p)
}
