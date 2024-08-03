// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use crate::Network;

#[test]
fn simple_in_order() {
    let log = Network::new_strings_run_script(
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
    let log = Network::new_strings_run_script(
        "
        modify add-bucket .
        modify fill-bucket .0 zero
        modify add-bucket .
        modify fill-bucket .1 one
        peek --apply 5
        peek --apply 5
        ",
    );
    insta::assert_ron_snapshot!(log, @r###"
    Log([
      BucketsNeedingFill([
        ".0",
      ]),
      BucketsNeedingFill([]),
      BucketsNeedingFill([
        ".1",
      ]),
      BucketsNeedingFill([]),
      Pop([
        "zero",
        "one",
        "zero",
        "one",
        "zero",
      ]),
      Pop([
        "one",
        "zero",
        "one",
        "zero",
        "one",
      ]),
    ])
    "###);
}

#[test]
fn depth_2() {
    let log = Network::new_strings_run_script(
        "
        modify add-bucket .
        modify fill-bucket .0 top-0-a top-0-b top-0-c
        modify add-joint .
        modify add-bucket .1
        modify fill-bucket .1.0 bot-1.0-a bot-1.0-b
        modify add-bucket .1
        modify fill-bucket .1.1 bot-1.1-a bot-1.1-b

        topology

        # peek only (no --apply)
        peek 4

        # peek with apply
        peek --apply 11

        peek --apply 5

        peek 0
        ",
    );
    insta::assert_ron_snapshot!(log, @r###"
    Log([
      BucketsNeedingFill([
        ".0",
      ]),
      BucketsNeedingFill([]),
      BucketsNeedingFill([
        ".1.0",
      ]),
      BucketsNeedingFill([]),
      BucketsNeedingFill([
        ".1.1",
      ]),
      BucketsNeedingFill([]),
      Topology([
        3,
        [
          2,
          2,
        ],
      ]),
      Peek([
        "top-0-a",
        "bot-1.0-a",
        "top-0-b",
        "bot-1.1-a",
      ]),
      Pop([
        "top-0-a",
        "bot-1.0-a",
        "top-0-b",
        "bot-1.1-a",
        "top-0-c",
        "bot-1.0-b",
        "top-0-a",
        "bot-1.1-b",
        "top-0-b",
        "bot-1.0-a",
        "top-0-c",
      ]),
      Pop([
        "bot-1.1-a",
        "top-0-a",
        "bot-1.0-b",
        "top-0-b",
        "bot-1.1-b",
      ]),
      Peek([]),
    ])
    "###);
}
