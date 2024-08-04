// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use super::arb_rng::{assert_arb_error, fake_rng};
use crate::Network;

#[test]
fn empty() {
    let network = Network::<(), ()>::default();
    arbtest::arbtest(|u| {
        let peeked = assert_arb_error(|| network.peek(&mut fake_rng(u), usize::MAX))?;
        assert_eq!(peeked.cancel_into_items(), Vec::<&()>::new());
        Ok(())
    });
}

#[test]
fn joint_filters() {
    let log = Network::<u8, i32>::default().run_script(
        "
        modify add-joint .
        modify set-joint-filters .0 1 2 3
        get-filters .0

        modify add-joint .0
        modify set-joint-filters -- .0.0 -4
        get-filters .0.0

        modify add-joint .0
        modify set-joint-filters .0.1 5
        get-filters .0.1

        modify set-joint-filters .0
        get-filters .0
        get-filters .0.0
        get-filters .0.1
        ",
    );
    insta::assert_ron_snapshot!(log, @r###"
    Log([
      BucketsNeedingFill("modify set-joint-filters .0 1 2 3"),
      Filters(".0", [
        [
          1,
          2,
          3,
        ],
      ]),
      BucketsNeedingFill("modify set-joint-filters -- .0.0 -4"),
      Filters(".0.0", [
        [
          1,
          2,
          3,
        ],
        [
          -4,
        ],
      ]),
      BucketsNeedingFill("modify set-joint-filters .0.1 5"),
      Filters(".0.1", [
        [
          1,
          2,
          3,
        ],
        [
          5,
        ],
      ]),
      BucketsNeedingFill("modify set-joint-filters .0"),
      Filters(".0", []),
      Filters(".0.0", [
        [
          -4,
        ],
      ]),
      Filters(".0.1", [
        [
          5,
        ],
      ]),
    ])
    "###);
}

#[test]
fn joint_filter_invalidates_buckets() {
    let log = Network::new_strings_run_script(
        "
        modify add-joint .
        modify add-bucket .0
        modify fill-bucket .0.0 item item2
        modify add-joint .0
        modify add-joint .0.1
        modify add-bucket .0.1.0
        modify fill-bucket .0.1.0.0 item item2

        modify set-joint-filters .0 filter-1 filter-2

        modify fill-bucket .0.0 item-modified
        modify fill-bucket .0.1.0.0 item-modified2
        ",
    );
    insta::assert_ron_snapshot!(log, @r###"
    Log([
      BucketsNeedingFill("modify add-bucket .0", [
        ".0.0",
      ]),
      BucketsNeedingFill("modify fill-bucket .0.0 item item2"),
      BucketsNeedingFill("modify add-bucket .0.1.0", [
        ".0.1.0.0",
      ]),
      BucketsNeedingFill("modify fill-bucket .0.1.0.0 item item2"),
      BucketsNeedingFill("modify set-joint-filters .0 filter-1 filter-2", [
        ".0.0",
        ".0.1.0.0",
      ]),
      BucketsNeedingFill("modify fill-bucket .0.0 item-modified", [
        ".0.1.0.0",
      ]),
      BucketsNeedingFill("modify fill-bucket .0.1.0.0 item-modified2"),
    ])
    "###);
}

#[test]
fn single_bucket() {
    let mut network = Network::<String, u8>::default();
    let log = network.run_script(
        "
        modify add-bucket .
        peek 9999
        modify fill-bucket .0 a b c
        ",
    );
    insta::assert_ron_snapshot!(log, @r###"
    Log([
      BucketsNeedingFill("modify add-bucket .", [
        ".0",
      ]),
      Peek([]),
      BucketsNeedingFill("modify fill-bucket .0 a b c"),
    ])
    "###);
}

#[test]
fn delete_empty_bucket() {
    let log = Network::new_strings_run_script(
        "
        modify add-bucket .
        modify fill-bucket .0 abc def

        !!expect_error delete non-empty bucket
        modify delete-empty .0

        modify fill-bucket .0
        modify delete-empty .0
        ",
    );
    insta::assert_ron_snapshot!(log, @r###"
    Log([
      BucketsNeedingFill("modify add-bucket .", [
        ".0",
      ]),
      BucketsNeedingFill("modify fill-bucket .0 abc def"),
      ExpectError("modify delete-empty .0", "cannot delete non-empty bucket: Path(.0)"),
      BucketsNeedingFill("modify fill-bucket .0"),
    ])
    "###);
}
#[test]
fn delete_empty_joint() {
    let log = Network::new_strings_run_script(
        "
        modify add-joint .
        modify add-joint .0

        !!expect_error delete non-empty joint
        modify delete-empty .0

        modify delete-empty .0.0
        modify delete-empty .0
        ",
    );
    insta::assert_ron_snapshot!(log, @r###"
    Log([
      ExpectError("modify delete-empty .0", "cannot delete non-empty joint: Path(.0)"),
    ])
    "###);
}

#[test]
fn fill_path_past_bucket() {
    let log = Network::new_strings_run_script(
        "
        modify add-bucket .
        !!expect_error fill path beyond bucket
        modify fill-bucket .0.0
        ",
    );
    insta::assert_ron_snapshot!(log, @r###"
    Log([
      BucketsNeedingFill("modify add-bucket .", [
        ".0",
      ]),
      ExpectError("modify fill-bucket .0.0", "unknown path: Path(.0.0)"),
    ])
    "###);
}
