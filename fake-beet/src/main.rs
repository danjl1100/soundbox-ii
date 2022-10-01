// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Stand-in / test-double for the `beet` command
//!
//! Outputs lines to stdout with the input arguments.
//!
//! When supplied an argument containing `fail`, the result is a failure.
//! If the final argument is a number, then that number of repeated lines will be output.
use std::{str::FromStr, time::Duration};

fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args()
        // remove executable name (passed as 0th arg)
        .skip(1)
        .collect();

    if let Some(first_arg) = args.first() {
        if first_arg == "--version" && args.len() == 1 {
            println!("beets version FAKE");
            return Ok(());
        }
    }

    let result = parse_result(&args);

    eprintln!("waiting...");
    std::thread::sleep(Duration::from_millis(800));

    if result.is_ok() {
        let count = parse_count(&args).unwrap_or(1);
        for n in 0..count {
            println!("fake-beet({current}/{count}) {args:?}", current = n + 1);
        }
        eprintln!("this std-err message should not appear in the output");
        eprintln!(" (since std-err is usually used for human-readable output)");
    } else {
        std::thread::sleep(Duration::from_millis(1500));
    }

    result
}

fn parse_count(args: &[String]) -> Option<u32> {
    u32::from_str(args.last()?).ok()
}

fn parse_result(args: &[String]) -> Result<(), String> {
    match args.iter().find_map(|arg| {
        arg.contains("fail").then_some(format!(
            "synthetic fail message triggered by arg {arg:?} out of {args:?}"
        ))
    }) {
        Some(error) => Err(error),
        None => Ok(()),
    }
}
