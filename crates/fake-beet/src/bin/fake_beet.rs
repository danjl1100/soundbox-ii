// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Binary for faking `beet` in end-to-end integration tests

fn main() -> eyre::Result<std::process::ExitCode> {
    fake_beet::fake_beet_main()
}
