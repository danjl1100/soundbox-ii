// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! One single entrypoint for the integration tests, to run all in parallel

#![allow(clippy::panic, reason = "expected in tests")]
#![allow(clippy::unwrap_used, reason = "allowed in tests")]

mod common {
    mod beet_to_vlc;
    mod end_to_end;
}

fn init_tracing() {
    use tracing_subscriber::{layer::SubscriberExt as _, util::SubscriberInitExt as _};
    let _ = tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .compact()
                .with_test_writer(),
        )
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();
}
