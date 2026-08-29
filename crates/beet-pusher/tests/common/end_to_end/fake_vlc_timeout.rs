// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use std::process::ExitStatus;

use serde_json::json;
use stdio_test::{ErrStr, JsonLines, Output};

use crate::{common::end_to_end::pipe_runner::PipeRunner, init_tracing};

/// Expected patterns for stderr
struct StderrPatterns;
impl StderrPatterns {
    const TIMEOUT: &[&str] = &[
        // rustfmt hint
        "failed evaluating endpoint",
        "timeout: global", // NOTE: relies on ureq specific error messages
    ];
    const UNAUTHORIZED: &[&str] = &[
        // rustfmt hint
        "failed evaluating endpoint",
        "http status: 401",
    ];

    fn all() -> impl Iterator<Item = &'static str> {
        let sets = [
            // rustfmt hint
            Self::TIMEOUT,
            Self::UNAUTHORIZED,
        ];
        sets.into_iter().flatten().copied()
    }
}

#[test]
fn pass() -> eyre::Result<()> {
    let test_case = TestCase {
        server_delay_millis: Some(20), // within the client timeout
        client_timeout_millis: Some(100),
        ..Default::default()
    };

    let TestOutput {
        vlc_json_str,
        vlc_requests_count,
        exit_status,
        stderr: ErrStr(stderr),
    } = test_case.run()?;

    assert!(
        exit_status.success(),
        "expected success for timeout above the server delay"
    );

    for stderr_pattern in StderrPatterns::all() {
        assert!(
            !stderr.contains(stderr_pattern),
            "unexpected error pattern in stderr: {stderr_pattern:?}"
        );
    }

    assert!(
        vlc_requests_count >= 5,
        "expected normal vlc_requests_count {vlc_requests_count}"
    );

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
    let test_case = TestCase {
        server_delay_millis: Some(150), // exceeds the client timeout
        client_timeout_millis: Some(100),
        ..Default::default()
    };

    let TestOutput {
        vlc_json_str,
        vlc_requests_count,
        exit_status,
        stderr: ErrStr(stderr),
    } = test_case.run()?;

    assert!(
        exit_status.success(),
        "expected non-fatal timeout errors to allow clean stdin-close shutdown"
    );

    for stderr_pattern in StderrPatterns::TIMEOUT {
        assert!(
            stderr.contains(stderr_pattern),
            "expected error pattern in stderr: {stderr_pattern:?}"
        );
    }

    assert!(
        vlc_requests_count > 1,
        "expected normal vlc_requests_count {vlc_requests_count}"
    );

    insta::assert_snapshot!(vlc_json_str, @"{}");

    Ok(())
}

#[test]
fn error_unauthorized() -> eyre::Result<()> {
    let test_case = TestCase {
        http_fail_code: Some(401),
        ..Default::default()
    };

    let TestOutput {
        vlc_json_str,
        vlc_requests_count,
        exit_status,
        stderr: ErrStr(stderr),
    } = test_case.run()?;

    assert!(
        !exit_status.success(),
        "expected fail for unauthorized response"
    );

    for stderr_pattern in StderrPatterns::UNAUTHORIZED {
        assert!(
            stderr.contains(stderr_pattern),
            "expected error pattern in stderr: {stderr_pattern:?}"
        );
    }

    assert_eq!(
        vlc_requests_count, 0,
        "expected single VLC request (no retries when unauthorized)"
    );

    insta::assert_snapshot!(vlc_json_str, @"{}");

    Ok(())
}

/// Parameters for fake-vlc timeout test
#[derive(Default)]
struct TestCase {
    server_delay_millis: Option<u8>,
    client_timeout_millis: Option<u8>,
    http_fail_code: Option<u16>,
}
struct TestOutput {
    vlc_json_str: String,
    vlc_requests_count: usize,
    exit_status: ExitStatus,
    stderr: ErrStr,
}
impl TestCase {
    fn run(self) -> eyre::Result<TestOutput> {
        let Self {
            server_delay_millis: response_delay_millis,
            client_timeout_millis: vlc_http_timeout_millis,
            http_fail_code,
        } = self;

        init_tracing();

        fake_vlc::FakeVlc::with_new_and_setup(
            |vlc| {
                if let Some(millis) = response_delay_millis {
                    let delay = std::time::Duration::from_millis(millis.into());
                    vlc.set_response_delay(delay);
                }

                if let Some(code) = http_fail_code {
                    vlc.set_http_fail_code(code);
                }
            },
            |vlc, _runner| {
                let fake_beet_config = fake_beet::ConfigAll::setup_with(|c| {
                    c.for_args(["ls", "-f$id=$path"]).stdout_lines(["1=item1"]);
                });
                let mut pipe_builder = PipeRunner::build(vlc, &fake_beet_config);

                if let Some(millis) = vlc_http_timeout_millis {
                    pipe_builder.set_vlc_http_timeout_millis(millis.into());
                }

                let mut r = pipe_builder.spawn()?;

                r.send_stdin(&JsonLines::new([json!({
                    "seq": 1,
                    "cmd": "add_node_to",
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
                    vlc_requests_count: vlc.get_requests_count(),
                    exit_status,
                    stderr,
                })
            },
        )
    }
}
