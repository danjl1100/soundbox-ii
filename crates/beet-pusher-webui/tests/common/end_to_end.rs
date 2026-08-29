// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use serde_json::json;
use stdio_test::{JsonLines, Output};

use crate::{
    common::end_to_end::pipe_runner::{PipeRunner, ReplyPlan},
    init_test_tracing,
};

mod pipe_runner;

#[test]
fn no_actions() -> eyre::Result<()> {
    init_test_tracing();

    let mut cmd = PipeRunner::spawn()?;

    let result = cmd.http_get("health")?;
    assert_eq!(result, "");

    let Output {
        stdout: _,
        stdout_json_lines,
        stderr: _,
    } = cmd.wait_success()??;

    JsonLines::new([]).check_eq_stdout(stdout_json_lines)??;

    Ok(())
}

#[test]
fn times_out() -> eyre::Result<()> {
    init_test_tracing();

    PipeRunner::spawn()?.run_then_wait_success(|cmd| {
        let result = cmd.http_post(
            "api/v1/nodes/create-bucket",
            &json!({
                "parent": ".",
            }),
        )?;
        assert!(result.contains(r#""status":"fail""#));
        Ok(())
    })?;

    Ok(())
}

#[test]
fn create_bucket() -> eyre::Result<()> {
    init_test_tracing();

    let plan_add_bucket = |seq, parent, created| ReplyPlan {
        request_pattern: json!({
            "seq": seq,
            "cmd": "add_node_to",
            "parent": parent,
            "node_kind": "bucket",
        }),
        response: json!({
            "data": {
                "reply_to_seq": seq,
                "kind": "node_added",
                "path": created,
            }
        }),
    };

    let cmd_result = PipeRunner::spawn()?.run_then_wait_success(|cmd| {
        cmd.queue_reply(plan_add_bucket(0, ".", ".0"))?;
        let result = cmd.http_post(
            "api/v1/nodes/create-bucket",
            &json!({
                "parent": ".",
            }),
        )?;
        insta::assert_snapshot!(result, @r#"{"status":"success","data":{"path":".0"}}"#);

        cmd.queue_reply(plan_add_bucket(1, ".", ".1"))?;
        let result = cmd.http_post(
            "api/v1/nodes/create-bucket",
            &json!({
                "parent": ".",
            }),
        )?;
        insta::assert_snapshot!(result, @r#"{"status":"success","data":{"path":".1"}}"#);

        cmd.queue_reply(plan_add_bucket(2, ".0", ".0.0"))?;
        let result = cmd.http_post(
            "api/v1/nodes/create-bucket",
            &json!({
                "parent": ".0",
            }),
        )?;
        insta::assert_snapshot!(result, @r#"{"status":"success","data":{"path":".0.0"}}"#);

        Ok(())
    });

    let Output {
        stdout: _,
        stdout_json_lines,
        stderr: _,
    } = cmd_result?.report_output()?;

    JsonLines::new([
        //
        json!({
            "seq": 0,
            "cmd": "add_node_to",
            "parent": ".",
            "node_kind": "bucket",
        }),
        json!({
            "seq": 1,
            "cmd": "add_node_to",
            "parent": ".",
            "node_kind": "bucket",
        }),
        json!({
            "seq": 2,
            "cmd": "add_node_to",
            "parent": ".0",
            "node_kind": "bucket",
        }),
    ])
    .check_eq_stdout(stdout_json_lines)??;

    Ok(())
}
