// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use super::arb_rng::{assert_arb_error, fake_rng};
use crate::{path::RemovedSelf, tests::script::NetworkStrings, ModifyErr, ModifyError, Network};

#[test]
fn empty() {
    let network = Network::<(), ()>::default();
    arbtest::arbtest(|u| {
        let peeked = assert_arb_error(network.peek(&mut fake_rng(u), usize::MAX))?;
        assert_eq!(peeked.cancel_into_items(), Vec::<&()>::new());
        Ok(())
    });
}

#[test]
fn joint_filters() -> eyre::Result<()> {
    let log = Network::<u8, i32>::default().run_script(
        "
        modify add-joint .
        modify set-filters .0 1 2 3
        get-filters .0

        modify add-joint .0
        modify set-filters -- .0.0 -4
        get-filters .0.0

        modify add-joint .0
        modify set-filters .0.1 5
        get-filters .0.1

        topology

        modify set-filters .0
        get-filters .0
        get-filters .0.0
        get-filters .0.1
        ",
    )?;
    insta::assert_ron_snapshot!(log, @r###"
    Log([
      BucketsNeedingFill("modify set-filters .0 1 2 3"),
      Filters(".0", [
        [
          1,
          2,
          3,
        ],
      ]),
      BucketsNeedingFill("modify set-filters -- .0.0 -4"),
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
      BucketsNeedingFill("modify set-filters .0.1 5"),
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
      Topology([
        [
          [],
          [],
        ],
      ]),
      BucketsNeedingFill("modify set-filters .0"),
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
    Ok(())
}

#[test]
fn bucket_filters() -> eyre::Result<()> {
    let log = Network::<u8, i32>::default().run_script(
        "
        modify add-joint .
        modify set-filters .0 254
        modify add-bucket .0
        modify set-filters -- .0.0 -9
        get-filters .0.0

        modify add-bucket .0
        modify set-filters -- .0.1 -4
        get-filters .0.1

        modify add-bucket .
        modify set-filters .1 5
        get-filters .1

        topology

        modify set-filters .0
        get-filters .0
        get-filters .0.0
        get-filters .0.1
        get-filters .1
        ",
    )?;
    insta::assert_ron_snapshot!(log, @r###"
    Log([
      BucketsNeedingFill("modify set-filters .0 254"),
      BucketsNeedingFill("modify add-bucket .0", [
        ".0.0",
      ]),
      BucketsNeedingFill("modify set-filters -- .0.0 -9", [
        ".0.0",
      ]),
      Filters(".0.0", [
        [
          254,
        ],
        [
          -9,
        ],
      ]),
      BucketsNeedingFill("modify add-bucket .0", [
        ".0.0",
        ".0.1",
      ]),
      BucketsNeedingFill("modify set-filters -- .0.1 -4", [
        ".0.0",
        ".0.1",
      ]),
      Filters(".0.1", [
        [
          254,
        ],
        [
          -4,
        ],
      ]),
      BucketsNeedingFill("modify add-bucket .", [
        ".0.0",
        ".0.1",
        ".1",
      ]),
      BucketsNeedingFill("modify set-filters .1 5", [
        ".0.0",
        ".0.1",
        ".1",
      ]),
      Filters(".1", [
        [
          5,
        ],
      ]),
      Topology([
        [
          0,
          0,
        ],
        0,
      ]),
      BucketsNeedingFill("modify set-filters .0", [
        ".0.0",
        ".0.1",
        ".1",
      ]),
      Filters(".0", []),
      Filters(".0.0", [
        [
          -9,
        ],
      ]),
      Filters(".0.1", [
        [
          -4,
        ],
      ]),
      Filters(".1", [
        [
          5,
        ],
      ]),
    ])
    "###);
    Ok(())
}

#[test]
fn joint_filter_invalidates_buckets() -> eyre::Result<()> {
    let log = Network::new_strings_run_script(
        "
        modify add-joint .
        modify add-bucket .0
        modify fill-bucket .0.0 item item2
        modify add-joint .0
        modify add-joint .0.1
        modify add-bucket .0.1.0
        modify fill-bucket .0.1.0.0 item item2

        modify set-filters .0 filter-1 filter-2

        modify fill-bucket .0.0 item-modified
        modify fill-bucket .0.1.0.0 item-modified2

        stats bucket-paths-map
        get-bucket-path 0
        get-bucket-path 1
        ",
    )?;
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
      BucketsNeedingFill("modify set-filters .0 filter-1 filter-2", [
        ".0.0",
        ".0.1.0.0",
      ]),
      BucketsNeedingFill("modify fill-bucket .0.0 item-modified", [
        ".0.1.0.0",
      ]),
      BucketsNeedingFill("modify fill-bucket .0.1.0.0 item-modified2"),
      InternalStats(BucketPathsMap(
        ids_needing_fill: [],
        cached_paths: [
          (BucketId(0), ".0.0"),
          (BucketId(1), ".0.1.0.0"),
        ],
      )),
      BucketPath(BucketId(0), ".0.0"),
      BucketPath(BucketId(1), ".0.1.0.0"),
    ])
    "###);
    Ok(())
}

#[test]
fn bucket_filter_invalidates_only_bucket() -> eyre::Result<()> {
    let log = Network::new_strings_run_script(
        "
        modify add-joint .
        modify add-bucket .0
        modify fill-bucket .0.0 item item2
        modify add-joint .0
        modify add-joint .0.1
        modify add-bucket .0.1.0
        modify fill-bucket .0.1.0.0 item item2

        topology

        modify set-filters .0.0 filter-1 filter-2
        modify fill-bucket .0.0 item-modified

        modify set-filters .0.1.0.0 filter-1 filter-2
        modify fill-bucket .0.1.0.0 item-modified2

        stats bucket-paths-map
        get-bucket-path 0
        get-bucket-path 1
        ",
    )?;
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
      Topology([
        [
          2,
          [
            [
              2,
            ],
          ],
        ],
      ]),
      BucketsNeedingFill("modify set-filters .0.0 filter-1 filter-2", [
        ".0.0",
      ]),
      BucketsNeedingFill("modify fill-bucket .0.0 item-modified"),
      BucketsNeedingFill("modify set-filters .0.1.0.0 filter-1 filter-2", [
        ".0.1.0.0",
      ]),
      BucketsNeedingFill("modify fill-bucket .0.1.0.0 item-modified2"),
      InternalStats(BucketPathsMap(
        ids_needing_fill: [],
        cached_paths: [
          (BucketId(0), ".0.0"),
          (BucketId(1), ".0.1.0.0"),
        ],
      )),
      BucketPath(BucketId(0), ".0.0"),
      BucketPath(BucketId(1), ".0.1.0.0"),
    ])
    "###);
    Ok(())
}

#[test]
fn single_bucket() -> eyre::Result<()> {
    let mut network = Network::<String, u8>::default();
    let log = network.run_script(
        "
        modify add-bucket .
        peek 9999
        modify fill-bucket .0 a b c
        ",
    )?;
    insta::assert_ron_snapshot!(log, @r###"
    Log([
      BucketsNeedingFill("modify add-bucket .", [
        ".0",
      ]),
      Peek([]),
      BucketsNeedingFill("modify fill-bucket .0 a b c"),
    ])
    "###);
    Ok(())
}

#[test]
fn delete_empty_bucket() -> eyre::Result<()> {
    let log = Network::new_strings_run_script(
        "
        modify add-bucket .
        modify fill-bucket .0 abc def

        !!expect_error delete non-empty bucket
        modify delete-empty .0

        modify fill-bucket .0
        modify delete-empty .0

        topology
        stats bucket-paths-map
        !!expect_error unknown bucket id
        get-bucket-path 1234567890
        ",
    )?;
    insta::assert_ron_snapshot!(log, @r###"
    Log([
      BucketsNeedingFill("modify add-bucket .", [
        ".0",
      ]),
      BucketsNeedingFill("modify fill-bucket .0 abc def"),
      ExpectError("modify delete-empty .0", "cannot delete non-empty bucket: Path(.0)"),
      BucketsNeedingFill("modify fill-bucket .0"),
      Topology([]),
      InternalStats(BucketPathsMap(
        ids_needing_fill: [],
        cached_paths: [],
      )),
      ExpectError("get-bucket-path 1234567890", "unknown bucket id: 1234567890"),
    ])
    "###);
    Ok(())
}
#[test]
fn delete_empty_joint() -> eyre::Result<()> {
    let log = Network::new_strings_run_script(
        "
        modify add-joint .
        modify add-joint .0

        !!expect_error delete non-empty joint
        modify delete-empty .0

        modify delete-empty .0.0
        modify delete-empty .0
        ",
    )?;
    insta::assert_ron_snapshot!(log, @r###"
    Log([
      ExpectError("modify delete-empty .0", "cannot delete non-empty joint: Path(.0)"),
    ])
    "###);
    Ok(())
}

#[test]
fn delete_updates_weights() -> eyre::Result<()> {
    let log = Network::new_strings_run_script(
        "
        modify add-bucket .
        modify add-joint .
        modify add-joint .1
        modify add-bucket .1

        modify set-weight .0 5
        modify set-weight .1.0 7

        topology weights

        modify delete-empty .0
        modify delete-empty .0.0

        topology weights

        stats bucket-paths-map
        get-bucket-path 1
        !!expect_error unknown bucket id
        get-bucket-path 0
        ",
    )?;
    insta::assert_ron_snapshot!(log, @r###"
    Log([
      BucketsNeedingFill("modify add-bucket .", [
        ".0",
      ]),
      BucketsNeedingFill("modify add-bucket .1", [
        ".0",
        ".1.1",
      ]),
      Topology([
        (5, ()),
        (1, [
          (7, []),
          (1, ()),
        ]),
      ]),
      Topology([
        (1, [
          (1, ()),
        ]),
      ]),
      InternalStats(BucketPathsMap(
        ids_needing_fill: [
          BucketId(1),
        ],
        cached_paths: [
          (BucketId(1), ".0.0"),
        ],
      )),
      BucketPath(BucketId(1), ".0.0"),
      ExpectError("get-bucket-path 0", "unknown bucket id: 0"),
    ])
    "###);
    Ok(())
}

#[test]
fn fill_path_past_bucket() -> eyre::Result<()> {
    let log = Network::new_strings_run_script(
        "
        modify add-bucket .
        !!expect_error fill path beyond bucket
        modify fill-bucket .0.0
        ",
    )?;
    insta::assert_ron_snapshot!(log, @r###"
    Log([
      BucketsNeedingFill("modify add-bucket .", [
        ".0",
      ]),
      ExpectError("modify fill-bucket .0.0", "unknown path: .0.0"),
    ])
    "###);
    Ok(())
}

#[test]
fn set_filter_past_bucket() -> eyre::Result<()> {
    let log = Network::new_strings_run_script(
        "
        modify add-bucket .
        !!expect_error set filter beyond bucket
        modify set-filters .0.0
        ",
    )?;
    insta::assert_ron_snapshot!(log, @r###"
    Log([
      BucketsNeedingFill("modify add-bucket .", [
        ".0",
      ]),
      ExpectError("modify set-filters .0.0", "unknown path: .0.0"),
    ])
    "###);
    Ok(())
}

#[test]
fn set_weights() -> eyre::Result<()> {
    let log = Network::new_strings_run_script(
        "
        modify add-joint .
        modify add-bucket .0
        modify add-bucket .

        topology weights

        modify set-weight .0 2
        modify set-weight .0.0 0
        modify set-weight .1 5

        topology weights
        ",
    )?;
    insta::assert_ron_snapshot!(log, @r###"
    Log([
      BucketsNeedingFill("modify add-bucket .0", [
        ".0.0",
      ]),
      BucketsNeedingFill("modify add-bucket .", [
        ".0.0",
        ".1",
      ]),
      Topology([
        (1, [
          (1, ()),
        ]),
        (1, ()),
      ]),
      Topology([
        (2, [
          (0, ()),
        ]),
        (5, ()),
      ]),
    ])
    "###);
    Ok(())
}

#[test]
fn delete_bucket_before_fill() -> eyre::Result<()> {
    let log = Network::new_strings_run_script(
        "
        modify add-bucket .
        modify add-joint .
        modify add-bucket .1

        modify delete-empty .0

        modify add-bucket .

        stats bucket-paths-map
        get-bucket-path 1
        get-bucket-path 2
        ",
    )?;
    insta::assert_ron_snapshot!(log, @r###"
    Log([
      BucketsNeedingFill("modify add-bucket .", [
        ".0",
      ]),
      BucketsNeedingFill("modify add-bucket .1", [
        ".0",
        ".1.0",
      ]),
      BucketsNeedingFill("modify add-bucket .", [
        ".0.0",
        ".1",
      ]),
      InternalStats(BucketPathsMap(
        ids_needing_fill: [
          BucketId(1),
          BucketId(2),
        ],
        cached_paths: [
          (BucketId(1), ".0.0"),
          (BucketId(2), ".1"),
        ],
      )),
      BucketPath(BucketId(1), ".0.0"),
      BucketPath(BucketId(2), ".1"),
    ])
    "###);
    Ok(())
}

#[test]
fn delete_then_view() -> eyre::Result<()> {
    let network = NetworkStrings::from_commands_str(
        "
        add-joint .
        add-joint .
        delete-empty .0
        ",
    )?;
    let view = network.view_table_default();
    println!("{view}");

    Ok(())
}

#[test]
fn delete_child_of_bucket() -> eyre::Result<()> {
    let log = Network::new_strings_run_script(
        "
        modify add-bucket .

        !!expect_error
        modify delete-empty .0.0

        !!expect_error
        modify delete-empty .5.6.7.8.9.10
        ",
    )?;
    insta::assert_ron_snapshot!(log, @r###"
    Log([
      BucketsNeedingFill("modify add-bucket .", [
        ".0",
      ]),
      ExpectError("modify delete-empty .0.0", "unknown path: .0.0"),
      ExpectError("modify delete-empty .5.6.7.8.9.10", "unknown path: .5.6.7.8.9.10"),
    ])
    "###);
    Ok(())
}

#[test]
fn set_weight_error() -> eyre::Result<()> {
    let log = Network::new_strings_run_script(
        "
        !!expect_error
        modify set-weight .4.5.6.7 1
        ",
    )?;
    insta::assert_ron_snapshot!(log, @r###"
    Log([
      ExpectError("modify set-weight .4.5.6.7 1", "unknown path: .4.5.6.7"),
    ])
    "###);
    Ok(())
}

#[test]
fn delete_from_arbitrary_network() {
    arbtest::arbtest(|u| {
        let mut network = Network::<String, String>::arbitrary(u)?;

        {
            let buckets = network.bucket_paths.expose_cache_for_test().count();
            let total = network.count_all_nodes();
            assert!(
                buckets <= total,
                "cached bucket count ({buckets}) should be <= total node count ({total})"
            );
        }

        let view = network.view_table_default();
        let mut paths: Vec<_> = view
            .get_rows()
            .iter()
            .flat_map(|row| {
                row.get_cells()
                    .iter()
                    .filter_map(|cell| cell.get_node().map(|cell| cell.get_path().to_owned()))
            })
            .collect();

        let iter_count = u.arbitrary_len::<usize>()?.max(paths.len());

        for _ in 0..iter_count {
            let path = paths.swap_remove(u.choose_index(paths.len())?);
            let cmd = crate::ModifyCmd::DeleteEmpty { path: path.clone() };
            let cmd_str = cmd.display_as_cmd_verified();
            println!("-> {cmd_str}");
            let result = network.modify(cmd);
            match result {
                Ok(()) => {
                    // shift down the affected entries
                    for p in &mut paths {
                        p.modify_for_removed(path.as_ref())
                            .unwrap_or_else(|_: RemovedSelf| {
                                panic!("removed path {path} should already be removed from paths")
                            });
                    }
                }
                Err(ModifyError(
                    ModifyErr::DeleteNonemptyBucket(_) | ModifyErr::DeleteNonemptyJoint(_),
                )) => {}
                Err(e) => {
                    let view = network.view_table_default();
                    println!("{view}");
                    panic!("unexpected error for {path} node removal: {e}")
                }
            }
        }
        Ok(())
    });
}
