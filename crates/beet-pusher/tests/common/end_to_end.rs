// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use crate::{
    common::end_to_end::pipe_runner::{JsonLines, Output, PipeRunner},
    init_tracing,
};

use serde_json::json;

mod fake_vlc_timeout;
mod pipe_runner;

#[test]
fn stdin_reports_unknown_command() -> eyre::Result<()> {
    fake_vlc::FakeVlc::with_new(|vlc, _runner| {
        let fake_beet_config = fake_beet::ConfigAll::default();

        let mut r = PipeRunner::build(vlc, &fake_beet_config).spawn()?;
        let bad_input = "test string is **NOT** a JSON object";
        r.send_stdin_line(bad_input)?;

        let Output {
            stdout,
            stdout_json_lines,
            stderr,
        } = r.wait_success()??;

        eprintln!("STDOUT:\n{stdout}\nEND");
        eprintln!("STDERR:\n{stderr}\nEND");

        JsonLines::one(json!({
            "error": {
                "kind": "invalid_command",
                "details": {
                    "command_json": bad_input,
                },
            },
        }))
        .check_eq_stdout(stdout_json_lines)??;

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
fn stdin_modify_spigot() -> eyre::Result<()> {
    init_tracing();

    fake_vlc::FakeVlc::with_new(|vlc, _runner| {
        let fake_beet_config = fake_beet::ConfigAll::setup_with(|c| {
            c.for_args(["ls", "-f$id=$path", "arg1", "arg2", "arg3"])
                .stdout_lines(["23=item1", "59=item2"]);
        });

        let mut r = PipeRunner::build(vlc, &fake_beet_config).spawn()?;
        r.send_stdin(&JsonLines::new([
            json!({
                "seq": 1,
                "cmd": "add_node_to",
                "parent": ".",
                "node_kind": "bucket",
            }),
            json!({
                "seq": 2,
                "cmd": "set_filters",
                "path": ".0",
                "new_filters": ["arg1", "arg2", "arg3"],
            }),
        ]))?;

        // fake VLC notifies the condvar when items are enqueued, so this returns ~100ms after
        // beet-pusher enqueues item1 (not after the full timeout)
        let advanced_to_item1 = vlc.wait_for_play_next(std::time::Duration::from_secs(1));

        // beet-pusher re-polls after ~500ms (WaitForVlc interval) and detects item1 playing,
        // then enqueues item2 ~100ms later via set_immediate
        std::thread::sleep(std::time::Duration::from_millis(800));

        r.send_stdin(&JsonLines::one(json!({
            "seq": 3,
            "cmd": "seek_next",
        })))?;

        let Output {
            stdout: _,
            stdout_json_lines,
            stderr,
        } = r.wait_success()??;

        eprintln!("STDERR:\n{stderr}\nEND");

        // begin behavior asserts (after printing stderr)
        advanced_to_item1.expect("advanced to item1");

        JsonLines::new([
            json!({
                "data": {
                    "reply_to_seq": 1,
                    "kind": "node_added",
                    "path": ".0",
                }
            }),
            json!({
                "data": {
                    "reply_to_seq": 2,
                    "kind": "pass",
                }
            }),
            json!({
                "data": {
                    "reply_to_seq": 3,
                    "kind": "pass",
                }
            }),
        ])
        .check_eq_stdout(stdout_json_lines)??;

        assert!(!stderr.to_lowercase().contains("error"), "error in stdout");

        insta::assert_snapshot!(vlc.get_json_str(), @r#"
        {
          "items": {
            "0": "file://base_url/item1",
            "1": "file://base_url/item2",
            "2": "file://base_url/item1"
          },
          "current_item_id": [
            1,
            "Playing"
          ]
        }
        "#);

        let playlist = vlc.get_playlist_cloned();
        assert_eq!(
            playlist,
            vec![
                vlc_http_test::model::Item {
                    id: 0, // NOTE: VLC Id, not the Beet ID
                    uri: "file://base_url/item1".to_string()
                },
                vlc_http_test::model::Item {
                    id: 1,
                    uri: "file://base_url/item2".to_string()
                },
                vlc_http_test::model::Item {
                    id: 2,
                    uri: "file://base_url/item1".to_string()
                },
            ]
        );

        Ok(())
    })
}
