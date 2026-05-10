// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use crate::common::end_to_end::pipe_runner::{JsonLines, Output, PipeRunner};

use serde_json::json;

mod pipe_runner;

fn base_beet_config(c: &mut fake_beet::ConfigAll) {
    // this is the current default script,
    // when changed then this will be emptier or completely unused
    c.for_args(["ls", "-f$id=$path", "grouping:1|2|3|4|5", "has_lyrics::^$"])
        .stdout_lines(["1=/path/to/file1.mp3", "2=/path/to/file2.mp3"]);
    c.for_args(["ls", "-f$id=$path", "added:2020..", "grouping::^$"])
        .stdout_lines(["5=/path/recent_file1.mp3", "6=/path/recent_file2.mp3"]);
    c.for_args(["ls", "-f$id=$path", "grouping::1|2|3|4|5", "has_lyrics::^$"])
        .stdout_lines(["7=/path/lyrics_file1.mp3"]);
}

#[test]
fn stdin_reports_unknown_command() -> eyre::Result<()> {
    fake_vlc::FakeVlc::with_new(|vlc, _runner| {
        let fake_beet_config = fake_beet::ConfigAll::setup_with(|c| {
            base_beet_config(c);
        });

        let mut r = PipeRunner::spawn(vlc, &fake_beet_config)?;
        let bad_input = "test string is **NOT** a JSON object";
        r.send_stdin_line(bad_input)?;

        let Output {
            stdout: _,
            stdout_json_lines,
            stderr,
        } = r.wait_success()?;

        JsonLines::one(json!({
            "error": {
                "kind": "invalid_command",
                "command_json": bad_input,
            },
        }))
        .assert_eq_stdout(stdout_json_lines)?;

        assert!(
            stderr.contains("test string is **NOT** a JSON object"),
            "expected library error in stderr"
        );
        assert!(
            stderr.contains("expected ident at line 1 column 2"),
            "expected JSON error in stderr:\n{stderr}"
        );

        Ok(())
    })
}

#[test]
#[ignore = "TODO"]
fn stdin_modify_spigot() -> eyre::Result<()> {
    fake_vlc::FakeVlc::with_new(|vlc, _runner| {
        let fake_beet_config = fake_beet::ConfigAll::setup_with(|c| {
            base_beet_config(c);
        });

        let mut r = PipeRunner::spawn(vlc, &fake_beet_config)?;
        r.send_stdin(&JsonLines::new([
            json!({
                "seq": 1,
                "cmd": "add_node",
                "parent": ".",
            }),
            json!({
                "seq": 2,
                "cmd": "set_filter",
                "path": ".0",
                "filters": ["arg1", "arg2", "arg3"],
            }),
        ]))?;

        let Output {
            stdout: _,
            stdout_json_lines,
            stderr,
        } = r.wait_success()?;

        eprintln!("STDERR:\n{stderr}\nEND");

        JsonLines::new([
            json!({
                "data": {
                    "reply_to_seq": 1,
                    "kind": "node_added",
                    "node": ".0",
                }
            }),
            json!({
                "data": {
                    "reply_to_seq": 2,
                    "kind": "generic",
                    "status": "pass",
                }
            }),
        ])
        .assert_eq_stdout(stdout_json_lines)?;

        assert!(!stderr.to_lowercase().contains("error"), "error in stdout");

        Ok(())
    })
}
