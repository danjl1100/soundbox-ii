// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

//! Verifies the items produced by peek/pop

use crate::Network;

#[test]
fn simple_in_order() -> eyre::Result<()> {
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
    )?;
    insta::assert_ron_snapshot!(log, @r###"
    Log([
      BucketsNeedingFill("modify add-bucket .", [
        ".0",
      ]),
      BucketsNeedingFill("modify fill-bucket .0 a b c"),
      Peek([
        "a",
        "b",
        "c",
      ]),
      BucketsNeedingFill("modify fill-bucket .0"),
      Peek([]),
    ])
    "###);
    Ok(())
}

#[test]
fn weighted_in_order() -> eyre::Result<()> {
    let log = Network::new_strings_run_script(
        "
        modify add-joint .
        modify add-joint .0
        modify add-bucket .0.0
        modify fill-bucket .0.0.0 nested-inner-1 nested-inner-2 nested-inner-3

        modify add-bucket .0
        modify fill-bucket .0.1 middle-1 middle-2 middle-3

        modify add-bucket .
        modify fill-bucket .1 base-1 base-2 base-3

        modify set-weight .0     4
        modify set-weight .0.0   2
        modify set-weight .0.0.0 200
        modify set-weight .1     1

        topology
        topology weights

        peek --show-bucket-ids 20
        ",
    )?;
    insta::assert_ron_snapshot!(log, @r###"
    Log([
      BucketsNeedingFill("modify add-bucket .0.0", [
        ".0.0.0",
      ]),
      BucketsNeedingFill("modify fill-bucket .0.0.0 nested-inner-1 nested-inner-2 nested-inner-3"),
      BucketsNeedingFill("modify add-bucket .0", [
        ".0.1",
      ]),
      BucketsNeedingFill("modify fill-bucket .0.1 middle-1 middle-2 middle-3"),
      BucketsNeedingFill("modify add-bucket .", [
        ".1",
      ]),
      BucketsNeedingFill("modify fill-bucket .1 base-1 base-2 base-3"),
      Topology([
        [
          [
            3,
          ],
          3,
        ],
        3,
      ]),
      Topology([
        (4, [
          (2, [
            (200, ()),
          ]),
          (1, ()),
        ]),
        (1, ()),
      ]),
      Peek([
        "nested-inner-1",
        "nested-inner-2",
        "middle-1",
        "nested-inner-3",
        "base-1",
        "nested-inner-1",
        "middle-2",
        "nested-inner-2",
        "nested-inner-3",
        "base-2",
        "middle-3",
        "nested-inner-1",
        "nested-inner-2",
        "middle-1",
        "base-3",
        "nested-inner-3",
        "nested-inner-1",
        "middle-2",
        "nested-inner-2",
        "base-1",
      ]),
      PopFrom([
        BucketId(0),
        BucketId(0),
        BucketId(1),
        BucketId(0),
        BucketId(2),
        BucketId(0),
        BucketId(1),
        BucketId(0),
        BucketId(0),
        BucketId(2),
        BucketId(1),
        BucketId(0),
        BucketId(0),
        BucketId(1),
        BucketId(2),
        BucketId(0),
        BucketId(0),
        BucketId(1),
        BucketId(0),
        BucketId(2),
      ]),
    ])
    "###);
    Ok(())
}

#[test]
fn two_alternating() -> eyre::Result<()> {
    let log = Network::new_strings_run_script(
        "
        modify add-bucket .
        modify fill-bucket .0 zero
        modify add-bucket .
        modify fill-bucket .1 one
        peek --apply --show-bucket-ids 5
        peek --apply --show-bucket-ids 5
        ",
    )?;
    insta::assert_ron_snapshot!(log, @r###"
    Log([
      BucketsNeedingFill("modify add-bucket .", [
        ".0",
      ]),
      BucketsNeedingFill("modify fill-bucket .0 zero"),
      BucketsNeedingFill("modify add-bucket .", [
        ".1",
      ]),
      BucketsNeedingFill("modify fill-bucket .1 one"),
      Pop([
        "zero",
        "one",
        "zero",
        "one",
        "zero",
      ]),
      PopFrom([
        BucketId(0),
        BucketId(1),
        BucketId(0),
        BucketId(1),
        BucketId(0),
      ]),
      Pop([
        "one",
        "zero",
        "one",
        "zero",
        "one",
      ]),
      PopFrom([
        BucketId(1),
        BucketId(0),
        BucketId(1),
        BucketId(0),
        BucketId(1),
      ]),
    ])
    "###);
    Ok(())
}

#[test]
fn depth_2() -> eyre::Result<()> {
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
        peek --apply --show-bucket-ids 11

        peek --apply --show-bucket-ids 5

        peek 0
        ",
    )?;
    insta::assert_ron_snapshot!(log, @r###"
    Log([
      BucketsNeedingFill("modify add-bucket .", [
        ".0",
      ]),
      BucketsNeedingFill("modify fill-bucket .0 top-0-a top-0-b top-0-c"),
      BucketsNeedingFill("modify add-bucket .1", [
        ".1.0",
      ]),
      BucketsNeedingFill("modify fill-bucket .1.0 bot-1.0-a bot-1.0-b"),
      BucketsNeedingFill("modify add-bucket .1", [
        ".1.1",
      ]),
      BucketsNeedingFill("modify fill-bucket .1.1 bot-1.1-a bot-1.1-b"),
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
      PopFrom([
        BucketId(0),
        BucketId(1),
        BucketId(0),
        BucketId(2),
        BucketId(0),
        BucketId(1),
        BucketId(0),
        BucketId(2),
        BucketId(0),
        BucketId(1),
        BucketId(0),
      ]),
      Pop([
        "bot-1.1-a",
        "top-0-a",
        "bot-1.0-b",
        "top-0-b",
        "bot-1.1-b",
      ]),
      PopFrom([
        BucketId(2),
        BucketId(0),
        BucketId(1),
        BucketId(0),
        BucketId(2),
      ]),
      Peek([]),
    ])
    "###);
    Ok(())
}

#[test]
fn continue_if_first_is_empty() -> eyre::Result<()> {
    let log = Network::new_strings_run_script(
        "
        # first is empty
        modify add-bucket .
        modify fill-bucket .0

        # second has items
        modify add-bucket .
        modify fill-bucket .1 a b c

        peek --show-bucket-ids 3
        ",
    )?;
    insta::assert_ron_snapshot!(log, @r###"
    Log([
      BucketsNeedingFill("modify add-bucket .", [
        ".0",
      ]),
      BucketsNeedingFill("modify fill-bucket .0"),
      BucketsNeedingFill("modify add-bucket .", [
        ".1",
      ]),
      BucketsNeedingFill("modify fill-bucket .1 a b c"),
      Peek([
        "a",
        "b",
        "c",
      ]),
      PopFrom([
        BucketId(1),
        BucketId(1),
        BucketId(1),
      ]),
    ])
    "###);
    Ok(())
}

#[test]
fn skips_empty_weight() -> eyre::Result<()> {
    let log = Network::new_strings_run_script(
        "
        modify add-bucket .
        modify fill-bucket .0 item-1
        modify add-bucket .
        modify fill-bucket .1 item-2

        modify set-weight .0 0
        topology weights

        peek --show-bucket-ids 4
        ",
    )?;
    insta::assert_ron_snapshot!(log, @r###"
    Log([
      BucketsNeedingFill("modify add-bucket .", [
        ".0",
      ]),
      BucketsNeedingFill("modify fill-bucket .0 item-1"),
      BucketsNeedingFill("modify add-bucket .", [
        ".1",
      ]),
      BucketsNeedingFill("modify fill-bucket .1 item-2"),
      Topology([
        (0, ()),
        (1, ()),
      ]),
      Peek([
        "item-2",
        "item-2",
        "item-2",
        "item-2",
      ]),
      PopFrom([
        BucketId(1),
        BucketId(1),
        BucketId(1),
        BucketId(1),
      ]),
    ])
    "###);
    Ok(())
}

#[test]
fn empty() -> eyre::Result<()> {
    let log = Network::new_strings_run_script("peek 1")?;
    insta::assert_ron_snapshot!(log, @r###"
    Log([
      Peek([]),
    ])
    "###);
    Ok(())
}

#[test]
fn sets_order_type_non_strict() -> eyre::Result<()> {
    let log = Network::new_strings_run_script(
        "
        enable-rng 0dfb8b701d6e8d57c83b0c9c6a92a16424fe

        modify add-bucket .
        modify fill-bucket .0 a b c d

        modify set-order-type .0 in-order
        peek 10

        modify set-order-type .0 shuffle
        peek 10

        modify set-order-type .0 random
        peek 10
        ",
    )?;
    insta::assert_ron_snapshot!(log, @r###"
    Log([
      BucketsNeedingFill("modify add-bucket .", [
        ".0",
      ]),
      BucketsNeedingFill("modify fill-bucket .0 a b c d"),
      Peek([
        "a",
        "b",
        "c",
        "d",
        "a",
        "b",
        "c",
        "d",
        "a",
        "b",
      ]),
      Peek([
        "b",
        "c",
        "d",
        "a",
        "a",
        "c",
        "d",
        "b",
        "b",
        "a",
      ]),
      Peek([
        "a",
        "d",
        "a",
        "a",
        "c",
        "c",
        "b",
        "a",
        "a",
        "c",
      ]),
    ])
    "###);
    Ok(())
}
