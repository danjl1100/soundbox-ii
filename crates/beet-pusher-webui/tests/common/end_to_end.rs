// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use serde_json::json;
use stdio_test::JsonLines;

use crate::common::end_to_end::pipe_runner::PipeRunner;

mod pipe_runner;

#[test]
#[ignore = "TODO"]
fn create_bucket() -> eyre::Result<()> {
    let mut cmd = PipeRunner::spawn()?;

    cmd.send_stdin(&JsonLines::one(json!({
        // TODO
    })))?;

    cmd.wait_success()??;

    Ok(())
}
