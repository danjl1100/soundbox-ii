// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

// teach me
#![deny(clippy::pedantic)]
#![allow(clippy::bool_to_int_with_if)] // except this confusing pattern
// no unsafe
#![forbid(unsafe_code)]
// no unwrap
#![deny(clippy::unwrap_used)]
// yes panic, it's tests! // no panic
// #![deny(clippy::panic)]
// docs!
#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

use test_log::test;
use tracing::{trace_span, warn_span};

mod common;

#[test]
fn test_cases() {
    insta::glob!("inputs/*.txt", test_case);
}

fn test_case(input: &std::path::Path) {
    let name = input
        .file_name()
        .expect("input file has a name")
        .to_str()
        .expect("test input filenames are UTF-8");
    let span = warn_span!("test_case", %name);
    let _guard = span.enter();

    let span = trace_span!("test_case", input_path=%input.display());
    let _guard = span.enter();

    let input = std::fs::read_to_string(input).expect("test input file exists");
    let output = common::run_input(&input);
    insta::assert_ron_snapshot!(output);
}
