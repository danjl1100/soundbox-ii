// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use serde_json::json;

use crate::{common::end_to_end::pipe_runner::PipeRunner, init_test_tracing};

mod pipe_runner;

#[test]
fn create_bucket() -> eyre::Result<()> {
    init_test_tracing();

    let mut cmd = PipeRunner::spawn()?;

    let result = cmd.http_post(
        "api/v1/nodes/create-bucket",
        &json!({
            "parent": ".",
        }),
    )?;
    insta::assert_snapshot!(result, @r#"{"status":"success","data":{"path":".0"}}"#);

    cmd.wait_success()??;

    Ok(())
}
