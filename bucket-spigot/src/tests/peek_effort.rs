// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

//! Verifies the [`Network::peek`] logic optimizes the "empty branch" case, by examining
//! deeply-nested buckets for varying peek lengths
//!
//! NOTE: All items are simple (`"a"`) since we're only testing the effort.

use crate::Network;

#[test]
fn repeat_empty() -> eyre::Result<()> {
    let log = Network::new_strings_run_script(
        "
        modify add-joint .
        modify add-joint .0
        modify add-joint .0.0
        modify add-bucket .0.0.0
        modify fill-bucket .0.0.0.0
        modify add-bucket .
        modify fill-bucket .1 a

        topology

        peek-assert --show-effort a
        peek-assert --show-effort a a a a a
        ",
    )?;
    insta::assert_ron_snapshot!(log, @r###"
    Log([
      BucketsNeedingFill("modify add-bucket .0.0.0", [
        ".0.0.0.0",
      ]),
      BucketsNeedingFill("modify fill-bucket .0.0.0.0"),
      BucketsNeedingFill("modify add-bucket .", [
        ".1",
      ]),
      BucketsNeedingFill("modify fill-bucket .1 a"),
      Topology([
        [
          [
            [
              0,
            ],
          ],
        ],
        1,
      ]),
      PeekEffort(6),
      PeekEffort(14),
    ])
    "###);
    Ok(())
}

fn test_case_nested(depth: usize, peek_2_exponent: u8) -> String {
    use std::fmt::Write as _;

    let mut script = String::new();

    writeln!(script, "# bury the `empty` bucket at depth={depth}").unwrap();
    for i in 0..depth {
        let parent_path: String = if i == 0 {
            ".".to_owned()
        } else {
            ".0".repeat(i)
        };
        writeln!(script, "modify add-joint  {parent_path}").unwrap();
    }

    let empty_parent = if depth == 0 {
        ".".to_owned()
    } else {
        ".0".repeat(depth)
    };
    let empty_path = ".0".repeat(depth + 1);
    writeln!(script, "modify add-bucket {empty_parent}").unwrap();

    let (filled_parent, filled_path) = if depth == 0 {
        (".", ".1")
    } else {
        (".0", ".0.1")
    }
    .to_owned();
    writeln!(script).unwrap();
    writeln!(script, "modify add-bucket {filled_parent}").unwrap();
    writeln!(script, "modify fill-bucket {filled_path} a").unwrap();

    writeln!(script).unwrap();
    writeln!(script, "topology").unwrap();

    let write_peeks = |script: &mut String| {
        writeln!(script).unwrap();
        for exponent in 0..=peek_2_exponent {
            let len = 2usize.pow(exponent.into());
            let expected = " a".repeat(len);
            writeln!(script, "# {len}").unwrap();
            writeln!(script, "peek-assert --show-effort{expected}").unwrap();
        }
    };

    write_peeks(&mut script);

    writeln!(script).unwrap();
    writeln!(script, "# fill nested bucket (less-optimizable case)").unwrap();
    writeln!(script, "modify fill-bucket {empty_path} a").unwrap();

    write_peeks(&mut script);

    script
}

#[test]
fn creates_script() -> eyre::Result<()> {
    let script = test_case_nested(0, 4);
    insta::assert_snapshot!(script, @r###"
    # bury the `empty` bucket at depth=0
    modify add-bucket .

    modify add-bucket .
    modify fill-bucket .1 a

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

    # fill nested bucket (less-optimizable case)
    modify fill-bucket .0 a

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
    "###);
    let log = Network::new_strings_run_script(&script)?;
    insta::assert_ron_snapshot!(log, @r###"
    Log([
      BucketsNeedingFill("modify add-bucket .", [
        ".0",
      ]),
      BucketsNeedingFill("modify add-bucket .", [
        ".0",
        ".1",
      ]),
      BucketsNeedingFill("modify fill-bucket .1 a", [
        ".0",
      ]),
      Topology([
        0,
        1,
      ]),
      PeekEffort(3),
      PeekEffort(5),
      PeekEffort(9),
      PeekEffort(17),
      PeekEffort(33),
      BucketsNeedingFill("modify fill-bucket .0 a"),
      PeekEffort(2),
      PeekEffort(4),
      PeekEffort(8),
      PeekEffort(16),
      PeekEffort(32),
    ])
    "###);
    Ok(())
}

#[test]
fn big_branch_8() -> eyre::Result<()> {
    let script = test_case_nested(8, 6);
    let log = Network::new_strings_run_script(&script)?;
    insta::assert_ron_snapshot!(log, @r###"
    Log([
      BucketsNeedingFill("modify add-bucket .0.0.0.0.0.0.0.0", [
        ".0.0.0.0.0.0.0.0.0",
      ]),
      BucketsNeedingFill("modify add-bucket .0", [
        ".0.0.0.0.0.0.0.0.0",
        ".0.1",
      ]),
      BucketsNeedingFill("modify fill-bucket .0.1 a", [
        ".0.0.0.0.0.0.0.0.0",
      ]),
      Topology([
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
          1,
        ],
      ]),
      PeekEffort(11),
      PeekEffort(14),
      PeekEffort(20),
      PeekEffort(32),
      PeekEffort(56),
      PeekEffort(104),
      PeekEffort(200),
      BucketsNeedingFill("modify fill-bucket .0.0.0.0.0.0.0.0.0 a"),
      PeekEffort(10),
      PeekEffort(13),
      PeekEffort(26),
      PeekEffort(52),
      PeekEffort(104),
      PeekEffort(208),
      PeekEffort(416),
    ])
    "###);
    Ok(())
}

#[test]
fn big_branch_16() -> eyre::Result<()> {
    let script = test_case_nested(16, 6);
    let log = Network::new_strings_run_script(&script)?;
    insta::assert_ron_snapshot!(log, @r###"
    Log([
      BucketsNeedingFill("modify add-bucket .0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0", [
        ".0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0",
      ]),
      BucketsNeedingFill("modify add-bucket .0", [
        ".0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0",
        ".0.1",
      ]),
      BucketsNeedingFill("modify fill-bucket .0.1 a", [
        ".0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0",
      ]),
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
      BucketsNeedingFill("modify fill-bucket .0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0 a"),
      PeekEffort(18),
      PeekEffort(21),
      PeekEffort(42),
      PeekEffort(84),
      PeekEffort(168),
      PeekEffort(336),
      PeekEffort(672),
    ])
    "###);
    Ok(())
}
