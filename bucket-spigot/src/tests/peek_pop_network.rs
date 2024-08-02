// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use crate::Network;

#[test]
fn simple_in_order() {
    let mut network = Network::new_strings();
    let log = network.run_script(
        "
        modify add-bucket .
        modify fill-bucket .0 a b c
        peek-assert a
        peek 3
        peek-assert a b
        peek-assert a b c
        peek-assert a b c a b c
        modify fill-bucket .0
        peek 1
        ",
    );
    insta::assert_ron_snapshot!(log, @r###"
    Log([
      BucketsNeedingFill([
        ".0",
      ]),
      BucketsNeedingFill([]),
      Peek([
        "a",
        "b",
        "c",
      ]),
      BucketsNeedingFill([]),
      Peek([]),
    ])
    "###);
}

#[test]
fn two_alternating() {
    let log = Network::new_strings().run_script(
        "
        modify add-bucket .
        modify add-bucket .
        modify fill-bucket .0 zero
        modify fill-bucket .1 one
        peek 5
        ",
    );
    insta::assert_ron_snapshot!(log, @r###"
    Log([
      BucketsNeedingFill([
        ".0",
      ]),
      BucketsNeedingFill([
        ".0",
        ".1",
      ]),
      BucketsNeedingFill([
        ".1",
      ]),
      BucketsNeedingFill([]),
      Peek([
        "zero",
        "one",
        "zero",
        "one",
        "zero",
      ]),
    ])
    "###);
}
