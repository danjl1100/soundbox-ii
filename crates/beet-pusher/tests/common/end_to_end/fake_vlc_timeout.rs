// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use std::process::ExitStatus;

use serde_json::json;
use stdio_test::{ErrStr, JsonLines, Output};

use crate::{common::end_to_end::pipe_runner::PipeRunner, init_tracing};

const STDERR_PATTERNS_FOR_TIMEOUT: &[&str] = &[
    // rustfmt hint
    "failed evaluating endpoint",
    "timeout: global", // NOTE: relies on ureq specific error messages
];

#[test]
fn pass() -> eyre::Result<()> {
    let TestOutput {
        vlc_json_str,
        exit_status,
        stderr: ErrStr(stderr),
    } = TestCase {
        server_delay_millis: 20, // within the client timeout
        client_timeout_millis: 100,
    }
    .run()?;

    assert!(
        exit_status.success(),
        "expected success for timeout above the server delay"
    );

    for stderr_pattern in STDERR_PATTERNS_FOR_TIMEOUT {
        assert!(
            !stderr.contains(stderr_pattern),
            "unexpected error pattern in stderr: {stderr_pattern:?}"
        );
    }

    insta::assert_snapshot!(vlc_json_str, @r#"
        {
          "items": {
            "0": "file://base_url/item1"
          }
        }
        "#);

    Ok(())
}
#[test]
fn error_timeout() -> eyre::Result<()> {
    let TestOutput {
        vlc_json_str,
        exit_status,
        stderr: ErrStr(stderr),
    } = TestCase {
        server_delay_millis: 150, // exceeds the client timeout
        client_timeout_millis: 100,
    }
    .run()?;

    assert!(
        !exit_status.success(),
        "expected fail for timeout below the server delay"
    );

    for stderr_pattern in STDERR_PATTERNS_FOR_TIMEOUT {
        assert!(
            stderr.contains(stderr_pattern),
            "expected error pattern in stderr: {stderr_pattern:?}"
        );
    }

    insta::assert_snapshot!(vlc_json_str, @"{}");

    Ok(())
}
/// Parameters for fake-vlc timeout test
struct TestCase {
    server_delay_millis: u8,
    client_timeout_millis: u8,
}
struct TestOutput {
    vlc_json_str: String,
    exit_status: ExitStatus,
    stderr: ErrStr,
}
impl TestCase {
    fn run(self) -> eyre::Result<TestOutput> {
        let Self {
            server_delay_millis: response_delay_millis,
            client_timeout_millis: vlc_http_timeout_millis,
        } = self;

        init_tracing();

        fake_vlc::FakeVlc::with_new_and_setup(
            |vlc| {
                vlc.set_response_delay(std::time::Duration::from_millis(
                    response_delay_millis.into(),
                ));
            },
            |vlc, _runner| {
                let fake_beet_config = fake_beet::ConfigAll::setup_with(|c| {
                    c.for_args(["ls", "-f$id=$path"]).stdout_lines(["1=item1"]);
                });
                let mut r = PipeRunner::build(vlc, &fake_beet_config)
                    .set_vlc_http_timeout_millis(vlc_http_timeout_millis.into())
                    .spawn()?;

                r.send_stdin(&JsonLines::new([json!({
                    "seq": 1,
                    "cmd": "add_node",
                    "parent": ".",
                    "node_kind": "bucket",
                })]))?;

                std::thread::sleep(std::time::Duration::from_millis(800));

                let (
                    Output {
                        stdout,
                        stdout_json_lines: _,
                        stderr,
                    },
                    exit_status,
                ) = r.wait_for_result()?;

                eprintln!("STDERR:\n{stderr}\nEND");
                eprintln!("STDOUT:\n{stdout}\nEND");

                Ok(TestOutput {
                    vlc_json_str: vlc.get_json_str(),
                    exit_status,
                    stderr,
                })
            },
        )
    }
}
