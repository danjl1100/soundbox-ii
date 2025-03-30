// Copyright (C) 2021-2025  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use std::path::{Path, PathBuf};
use std::process::Command;

fn get_fakebeet_path() -> String {
    let fakebeet = "../fake-beet/target/debug/fake-beet";
    if !PathBuf::from(fakebeet).exists() {
        panic!("fakebeet not found at path {fakebeet:?}");
    }
    fakebeet.to_string()
}

fn run_script<T>(script: impl AsRef<Path>, extra_args: Vec<T>) -> std::process::Output
where
    T: Into<String>,
{
    let fakebeet = get_fakebeet_path();

    let args = vec![
        "--beet-cmd".to_string(),
        fakebeet,
        // "--source-type".to_string(),
        // "folder-listing".to_string(),
        "--script".to_string(),
        script.as_ref().to_string_lossy().to_string(),
        "--quiet".to_string(),
    ];
    let args = args
        .into_iter()
        .chain(extra_args.into_iter().map(Into::into));

    let bin_sequencer = PathBuf::from(env!("CARGO_BIN_EXE_sequencer"));

    Command::new(bin_sequencer)
        .args(args)
        .output()
        .expect("failed to execute")
}

fn output_to_str_lossy(output: &std::process::Output) -> String {
    format!(
        "---STDOUT---\n{stdout}---STDERR---\n{stderr}",
        stdout = String::from_utf8_lossy(&output.stdout),
        stderr = String::from_utf8_lossy(&output.stderr),
    )
}

const ARGS_SOURCE_FOLDER_LISTING: &[&str] = &["--source-type", "folder-listing"];

#[test]
#[ignore = "fake_beet not hooked up for nix builds"]
fn sequencer_cli() {
    let output = run_script("src/test_script.txt", ARGS_SOURCE_FOLDER_LISTING.to_vec());

    let output_str = output_to_str_lossy(&output);

    assert!(
        output.status.success(),
        "test command failed, inspect output: \n{output_str}",
    );
}

#[test]
#[ignore = "fake_beet not hooked up for nix builds"]
fn sequencer_cli_move() {
    let output = run_script(
        "src/test_script_move.txt",
        ARGS_SOURCE_FOLDER_LISTING.to_vec(),
    );

    let output_str = output_to_str_lossy(&output);

    assert!(
        output.status.success(),
        "test command failed, inspect output: \n{output_str}",
    );
}

#[test]
#[ignore = "kdl serde bridge is incomplete"]
fn sequencer_cli_persist() {
    let output = run_script(
        "src/test_script_persist.txt",
        ARGS_SOURCE_FOLDER_LISTING.to_vec(),
    );

    let output_str = output_to_str_lossy(&output);

    assert!(
        output.status.success(),
        "test command failed, inspect output: \n{output_str}",
    );
}
