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

#[test]
fn continue_if_first_is_empty() {
    let log = Network::new_strings_run_script(
        "
        # first is empty
        modify add-bucket .
        modify fill-bucket .0

        # second has items
        modify add-bucket .
        modify fill-bucket .1 a b c

        peek 3
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
      Peek([
        "a",
        "b",
        "c",
      ]),
    ])
    "###);
}

#[test]
fn skips_effort_repeat_empty() {
    let log = Network::new_strings_run_script(
        "
        modify add-joint .
        modify add-joint .0
        modify add-joint .0.0
        modify add-bucket .
        modify fill-bucket .1 item

        topology

        peek --show-effort 1
        peek --show-effort 5
        ",
    );
    insta::assert_ron_snapshot!(log, @r###"
    Log([
      BucketsNeedingFill([
        ".1",
      ]),
      BucketsNeedingFill([]),
      Topology([
        [
          [
            [],
          ],
        ],
        1,
      ]),
      Peek(5, [
        "item",
      ]),
      Peek(13, [
        "item",
        "item",
        "item",
        "item",
        "item",
      ]),
    ])
    "###);
}

#[test]
fn skips_effort_repeat_empty_big_branch() {
    let log = Network::new_strings_run_script(
        "
        modify add-joint  .
        modify add-joint  .0
        modify add-joint  .0.0
        modify add-joint  .0.0.0
        modify add-joint  .0.0.0.0
        modify add-joint  .0.0.0.0.0
        modify add-joint  .0.0.0.0.0.0
        modify add-joint  .0.0.0.0.0.0.0
        modify add-joint  .0.0.0.0.0.0.0.0
        modify add-joint  .0.0.0.0.0.0.0.0.0
        modify add-joint  .0.0.0.0.0.0.0.0.0.0
        modify add-joint  .0.0.0.0.0.0.0.0.0.0.0
        modify add-joint  .0.0.0.0.0.0.0.0.0.0.0.0
        modify add-joint  .0.0.0.0.0.0.0.0.0.0.0.0.0
        modify add-joint  .0.0.0.0.0.0.0.0.0.0.0.0.0.0
        modify add-joint  .0.0.0.0.0.0.0.0.0.0.0.0.0.0.0
        modify add-bucket .0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0
        modify fill-bucket .0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0
        modify add-bucket .0
        modify fill-bucket .0.1 a

        topology

        # 1
        peek-assert --show-effort a
        # 2
        peek-assert --show-effort a a
        # 4
        peek-assert --show-effort a a a a
        # 8
        peek-assert --show-effort a a a a a a a a
        # 16
        peek-assert --show-effort a a a a a a a a a a a a a a a a
        # 32
        peek-assert --show-effort a a a a a a a a a a a a a a a a a a a a a a a a a a a a a a a a
        # 64
        peek-assert --show-effort a a a a a a a a a a a a a a a a a a a a a a a a a a a a a a a a a a a a a a a a a a a a a a a a a a a a a a a a a a a a a a a a
        ",
    );
    insta::assert_ron_snapshot!(log, @r###"
    Log([
      BucketsNeedingFill([
        ".0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0",
      ]),
      BucketsNeedingFill([]),
      BucketsNeedingFill([
        ".0.1",
      ]),
      BucketsNeedingFill([]),
      Topology([
        [
          [
            [
              [
                [
                  [
                    [
                      [
                        [
                          [
                            [
                              [
                                [
                                  [
                                    [
                                      [
                                        0,
                                      ],
                                    ],
                                  ],
                                ],
                              ],
                            ],
                          ],
                        ],
                      ],
                    ],
                  ],
                ],
              ],
            ],
          ],
          1,
        ],
      ]),
      PeekEffort(19),
      PeekEffort(22),
      PeekEffort(28),
      PeekEffort(40),
      PeekEffort(64),
      PeekEffort(112),
      PeekEffort(208),
    ])
    "###);
}
