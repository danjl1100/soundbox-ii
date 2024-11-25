// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use crate::{
    path::{Path, PathRef},
    tests::script::NetworkStrings,
    view::TableParams,
    Network,
};
use std::str::FromStr as _;

#[test]
fn empty() {
    let network = NetworkStrings::default();
    let table = network.view_table_default();
    insta::assert_snapshot!(table, @r###"
    Table {
    }
    "###);
}

#[test]
fn table_weights() -> eyre::Result<()> {
    let mut network = NetworkStrings::from_commands_str(
        "
        add-bucket .
        fill-bucket .0 abc def ghi jkl
        add-joint .
        add-joint .1
        add-bucket .1
        fill-bucket .1.1 qrs tuv wxyz

        add-joint .1.0
        add-bucket .1.0
        fill-bucket .1.0.1 1 2 3 4
        add-bucket .1.0
        fill-bucket .1.0.2 5 6 7 8 9
        ",
    )?;
    let log = network.run_script("topology");
    insta::assert_ron_snapshot!(log.items().last().unwrap(), @r###"
    Topology([
      4,
      [
        [
          [],
          4,
          5,
        ],
        3,
      ],
    ])
    "###);

    let params = TableParams::default();

    let table = network.view_table(params).unwrap();
    insta::assert_snapshot!(table, @r###"
    Table {
    X <------- .0 bucket (4 items) in order
     XXXX <--- .1 joint (2 children) in order
     XXX <---- .1.0 joint (3 children) in order
        X <--- .1.1 bucket (3 items) in order
     X <------ .1.0.0 joint (empty) in order
      X <----- .1.0.1 bucket (4 items) in order
       X <---- .1.0.2 bucket (5 items) in order
    }
    "###);

    network.run_script(
        "
        modify set-weight .1.1 0
        ",
    );
    let table = network.view_table(params).unwrap();
    insta::assert_snapshot!(table, @r###"
    Table {
    X <------- .0 bucket (4 items) in order
     XXXX <--- .1 joint (2 children) in order
     XXX <---- .1.0 x1 joint (3 children) in order
        o <--- .1.1 x0 bucket (3 items) in order (inactive)
     X <------ .1.0.0 joint (empty) in order
      X <----- .1.0.1 bucket (4 items) in order
       X <---- .1.0.2 bucket (5 items) in order
    }
    "###);
    network.run_script(
        "
        modify set-weight .1.0 0
        ",
    );
    let table = network.view_table(params).unwrap();
    insta::assert_snapshot!(table, @r###"
    Table {
    X <------- .0 bucket (4 items) in order
     XXXX <--- .1 joint (2 children) in order
     ooo <---- .1.0 x0 joint (3 children) in order (inactive)
        o <--- .1.1 x0 bucket (3 items) in order (inactive)
     o <------ .1.0.0 joint (empty) in order (inactive)
      o <----- .1.0.1 bucket (4 items) in order (inactive)
       o <---- .1.0.2 bucket (5 items) in order (inactive)
    }
    "###);

    Ok(())
}

fn arbitrary_pattern1() -> eyre::Result<NetworkStrings> {
    Ok(NetworkStrings::from_commands_str(
        "
        add-joint .
        add-bucket .
        add-bucket .
        add-bucket .
        add-joint .

        add-joint .0
        add-bucket .0

        add-bucket .0.0
        add-bucket .0.0

        add-bucket .4
        add-joint .4

        add-bucket .4.1
        add-bucket .4.1

        set-weight .0 0
        set-weight .0.1 50
        set-weight .1 2
        set-weight .2 3
        set-weight .3 4
        ",
    )?)
}

#[test]
fn table_depth_root() -> eyre::Result<()> {
    let network = arbitrary_pattern1()?;

    let params = TableParams::default();

    let root_max = network.view_table(params).unwrap();
    let root_depth_2 = network.view_table(params.set_max_depth(2)).unwrap();
    let root_depth_1 = network.view_table(params.set_max_depth(1)).unwrap();
    let root_depth_0 = network.view_table(params.set_max_depth(0)).unwrap();
    assert_eq!(root_max, root_depth_2);
    insta::assert_snapshot!(root_depth_2, @r###"
    Table {
    ooo <--------- .0 x0 joint (2 children) in order (inactive)
       X <-------- .1 x2 bucket (empty) in order
        X <------- .2 x3 bucket (empty) in order
         X <------ .3 x4 bucket (empty) in order
          XXX <--- .4 x1 joint (2 children) in order
    oo <---------- .0.0 x1 joint (2 children) in order (inactive)
      o <--------- .0.1 x50 bucket (empty) in order (inactive)
          X <----- .4.0 bucket (empty) in order
           XX <--- .4.1 joint (2 children) in order
    o <----------- .0.0.0 bucket (empty) in order (inactive)
     o <---------- .0.0.1 bucket (empty) in order (inactive)
           X <---- .4.1.0 bucket (empty) in order
            X <--- .4.1.1 bucket (empty) in order
    }
    "###);

    insta::assert_snapshot!(root_depth_1, @r###"
    Table {
    oo <-------- .0 x0 joint (2 children) in order (inactive)
      X <------- .1 x2 bucket (empty) in order
       X <------ .2 x3 bucket (empty) in order
        X <----- .3 x4 bucket (empty) in order
         XX <--- .4 x1 joint (2 children) in order
    o <--------- .0.0 x1 joint (2 children hidden) in order (inactive)
     o <-------- .0.1 x50 bucket (empty) in order (inactive)
         X <---- .4.0 bucket (empty) in order
          X <--- .4.1 joint (2 children hidden) in order
    }
    "###);

    insta::assert_snapshot!(root_depth_0, @r###"
    Table {
    o <------- .0 x0 joint (2 children hidden) in order (inactive)
     X <------ .1 x2 bucket (empty) in order
      X <----- .2 x3 bucket (empty) in order
       X <---- .3 x4 bucket (empty) in order
        X <--- .4 x1 joint (2 children hidden) in order
    }
    "###);

    insta::assert_snapshot!(view_path(&network, ".0.1"), @r###"
    Table {
    o <--- .0.1 x50 bucket (empty) in order (inactive)
    }
    "###);
    insta::assert_snapshot!(view_path(&network, ".0.0.1"), @r###"
    Table {
    o <--- .0.0.1 bucket (empty) in order (inactive)
    }
    "###);

    Ok(())
}
#[test]
fn table_depths_narrow_to_wider() -> eyre::Result<()> {
    let network = arbitrary_pattern1()?;

    insta::assert_snapshot!(view_path(&network, ".4.1.1"), @r###"
    Table {
    X <--- .4.1.1 bucket (empty) in order
    }
    "###);
    println!("--------------------------------------------------");
    insta::assert_snapshot!(view_path(&network, ".4.1.0"), @r###"
    Table {
    X <---- .4.1.0 bucket (empty) in order
     X <--- .4.1.1 bucket (empty) in order
    }
    "###);
    insta::assert_snapshot!(view_path(&network, ".4.1"), @r###"
    Table {
    XX <--- .4.1 joint (2 children) in order
    X <---- .4.1.0 bucket (empty) in order
     X <--- .4.1.1 bucket (empty) in order
    }
    "###);
    insta::assert_snapshot!(view_path(&network, ".4.0"), @r###"
    Table {
    X <----- .4.0 bucket (empty) in order
     XX <--- .4.1 joint (2 children) in order
     X <---- .4.1.0 bucket (empty) in order
      X <--- .4.1.1 bucket (empty) in order
    }
    "###);
    insta::assert_snapshot!(view_path(&network, ".4"), @r###"
    Table {
    XXX <--- .4 x1 joint (2 children) in order
    X <----- .4.0 bucket (empty) in order
     XX <--- .4.1 joint (2 children) in order
     X <---- .4.1.0 bucket (empty) in order
      X <--- .4.1.1 bucket (empty) in order
    }
    "###);

    Ok(())
}
#[test]
fn simple_gap() -> eyre::Result<()> {
    let mut network = NetworkStrings::from_commands_str(
        "
        add-joint .
        add-joint .
        add-joint .

        add-joint .0
        add-joint .2
        ",
    )?;
    let log = network.run_script("topology");
    insta::assert_ron_snapshot!(log, @r###"
    Log([
      Topology([
        [
          [],
        ],
        [],
        [
          [],
        ],
      ]),
    ])
    "###);

    insta::assert_snapshot!(view_path(&network, "."), @r###"
    Table {
    X <----- .0 joint (1 child) in order
     X <---- .1 joint (empty) in order
      X <--- .2 joint (1 child) in order
    X <----- .0.0 joint (empty) in order
      X <--- .2.0 joint (empty) in order
    }
    "###);

    Ok(())
}
#[test]
fn simple_max_depth() -> eyre::Result<()> {
    let mut network = NetworkStrings::from_commands_str(
        "
        add-joint .
        add-joint .0
        add-joint .0.0
        ",
    )?;
    let log = network.run_script("topology");
    insta::assert_ron_snapshot!(log, @r###"
    Log([
      Topology([
        [
          [
            [],
          ],
        ],
      ]),
    ])
    "###);

    let params = TableParams::default();

    let root_depth_2 = network.view_table(params.set_max_depth(2)).unwrap();
    insta::assert_snapshot!(root_depth_2, @r###"
    Table {
    X <--- .0 joint (1 child) in order
    X <--- .0.0 joint (1 child) in order
    X <--- .0.0.0 joint (empty) in order
    }
    "###);
    let root_depth_1 = network.view_table(params.set_max_depth(1)).unwrap();
    insta::assert_snapshot!(root_depth_1, @r###"
    Table {
    X <--- .0 joint (1 child) in order
    X <--- .0.0 joint (1 child hidden) in order
    }
    "###);
    let root_depth_0 = network.view_table(params.set_max_depth(0)).unwrap();
    insta::assert_snapshot!(root_depth_0, @r###"
    Table {
    X <--- .0 joint (1 child hidden) in order
    }
    "###);

    Ok(())
}

fn view_path<T, U>(network: &Network<T, U>, path_str: &str) -> String {
    let path = Path::from_str(path_str).unwrap();
    let params = TableParams::default().set_base_path(path.as_ref());
    network.view_table(params).unwrap().to_string()
}

#[test]
fn unique_weights() -> eyre::Result<()> {
    let network = NetworkStrings::from_commands_str(
        "
        add-joint .
        add-joint .
        add-joint .
        add-joint .
        add-joint .

        add-joint .0
        add-joint .0
        add-joint .0
        add-joint .0
        add-joint .0

        set-weight .0 1
        set-weight .1 2
        set-weight .2 3
        set-weight .3 4
        set-weight .4 5
        set-weight .0.0 6
        set-weight .0.1 7
        set-weight .0.2 8
        set-weight .0.3 9
        set-weight .0.4 10
        ",
    )?;
    insta::assert_snapshot!(view_path(&network, ".0"), @r###"
    Table {
    XXXXX <------- .0 x1 joint (5 children) in order
         X <------ .1 x2 joint (empty) in order
          X <----- .2 x3 joint (empty) in order
           X <---- .3 x4 joint (empty) in order
            X <--- .4 x5 joint (empty) in order
    X <----------- .0.0 x6 joint (empty) in order
     X <---------- .0.1 x7 joint (empty) in order
      X <--------- .0.2 x8 joint (empty) in order
       X <-------- .0.3 x9 joint (empty) in order
        X <------- .0.4 x10 joint (empty) in order
    }
    "###);
    insta::assert_snapshot!(view_path(&network, ".1"), @r###"
    Table {
    X <------ .1 x2 joint (empty) in order
     X <----- .2 x3 joint (empty) in order
      X <---- .3 x4 joint (empty) in order
       X <--- .4 x5 joint (empty) in order
    }
    "###);
    insta::assert_snapshot!(view_path(&network, ".3"), @r###"
    Table {
    X <---- .3 x4 joint (empty) in order
     X <--- .4 x5 joint (empty) in order
    }
    "###);
    insta::assert_snapshot!(view_path(&network, ".4"), @r###"
    Table {
    X <--- .4 x5 joint (empty) in order
    }
    "###);
    insta::assert_snapshot!(view_path(&network, ".0.0"), @r###"
    Table {
    X <------- .0.0 x6 joint (empty) in order
     X <------ .0.1 x7 joint (empty) in order
      X <----- .0.2 x8 joint (empty) in order
       X <---- .0.3 x9 joint (empty) in order
        X <--- .0.4 x10 joint (empty) in order
    }
    "###);
    insta::assert_snapshot!(view_path(&network, ".0.1"), @r###"
    Table {
    X <------ .0.1 x7 joint (empty) in order
     X <----- .0.2 x8 joint (empty) in order
      X <---- .0.3 x9 joint (empty) in order
       X <--- .0.4 x10 joint (empty) in order
    }
    "###);
    insta::assert_snapshot!(view_path(&network, ".0.3"), @r###"
    Table {
    X <---- .0.3 x9 joint (empty) in order
     X <--- .0.4 x10 joint (empty) in order
    }
    "###);
    insta::assert_snapshot!(view_path(&network, ".0.4"), @r###"
    Table {
    X <--- .0.4 x10 joint (empty) in order
    }
    "###);

    Ok(())
}

#[test]
fn table_depth_child_right() -> eyre::Result<()> {
    let network = arbitrary_pattern1()?;

    let params = TableParams::default();
    let path = Path::from_str(".4").unwrap();
    let params = params.set_base_path(path.as_ref());

    let right_max = network.view_table(params).unwrap();
    let right_depth_2 = network.view_table(params.set_max_depth(2)).unwrap();
    let right_depth_1 = network.view_table(params.set_max_depth(1)).unwrap();
    let right_depth_0 = network.view_table(params.set_max_depth(0)).unwrap();
    assert_eq!(right_max, right_depth_2);
    insta::assert_snapshot!(right_depth_2, @r###"
    Table {
    XXX <--- .4 x1 joint (2 children) in order
    X <----- .4.0 bucket (empty) in order
     XX <--- .4.1 joint (2 children) in order
     X <---- .4.1.0 bucket (empty) in order
      X <--- .4.1.1 bucket (empty) in order
    }
    "###);
    insta::assert_snapshot!(right_depth_1, @r###"
    Table {
    XX <--- .4 x1 joint (2 children) in order
    X <---- .4.0 bucket (empty) in order
     X <--- .4.1 joint (2 children hidden) in order
    }
    "###);
    insta::assert_snapshot!(right_depth_0, @r###"
    Table {
    X <--- .4 x1 joint (2 children hidden) in order
    }
    "###);

    Ok(())
}

#[test]
fn table_view_bucket() -> eyre::Result<()> {
    let mut network = NetworkStrings::from_commands_str(
        "
        add-joint .
        add-bucket .
        add-bucket .0
        ",
    )?;
    let path_1 = Path::from_str(".1").unwrap();
    let path_2 = Path::from_str(".0.0").unwrap();
    let params_1 = TableParams::default().set_base_path(path_1.as_ref());
    let params_2 = TableParams::default().set_base_path(path_2.as_ref());
    insta::assert_snapshot!(network.view_table(params_1).unwrap(), @r###"
    Table {
    X <--- .1 bucket (empty) in order
    }
    "###);
    insta::assert_snapshot!(network.view_table(params_2).unwrap(), @r###"
    Table {
    X <--- .0.0 bucket (empty) in order
    }
    "###);

    network.run_script("modify set-weight .0 0");
    insta::assert_snapshot!(network.view_table(params_2).unwrap(), @r###"
    Table {
    o <--- .0.0 bucket (empty) in order (inactive)
    }
    "###);
    Ok(())
}

/// Add `count` child joints to specified node
fn fill_width_at<T, U>(network: &mut Network<T, U>, parent: PathRef<'_>, count: usize) {
    for _ in 0..count {
        network
            .modify(crate::ModifyCmd::AddJoint {
                parent: parent.to_owned(),
            })
            .unwrap();
    }
}
// Add `count` joints as child chain from the first node
fn fill_depth_at<T, U>(network: &mut Network<T, U>, parent: Path, count: usize) {
    let mut depth_path = parent;
    for _ in 0..count {
        network
            .modify(crate::ModifyCmd::AddJoint {
                parent: depth_path.clone(),
            })
            .unwrap();
        depth_path.push(0);
    }
}

fn fill_width_and_depth<T, U>(network: &mut Network<T, U>, parent: Path, count: usize) {
    fill_width_at(network, parent.as_ref(), count);

    let below_parent = {
        let mut below_parent = parent;
        below_parent.push(0);
        below_parent
    };
    fill_depth_at(network, below_parent, count - 1);
}

#[test]
fn limit_width_root() {
    const N: usize = 50;
    let mut network = NetworkStrings::default();

    let root = Path::empty();
    fill_width_and_depth(&mut network, root, N);

    // Assert full view is LONG
    let params = TableParams::default();
    let full = network.view_table(params).unwrap();
    insta::assert_snapshot!(full);

    // Assert shortened view
    let params_width_2 = params.set_max_depth(2).set_max_width(2);
    let width_2 = network.view_table(params_width_2).unwrap();
    insta::assert_snapshot!(width_2, @r###"
    Table {
    X <---- .0 joint (1 child) in order
     X <--- .1 joint (empty) in order
      ? <--- (one or more nodes omitted...)
    X <---- .0.0 joint (1 child) in order
    X <---- .0.0.0 joint (1 child hidden) in order
    }
    "###);

    let offset = Path::from_str(".2").unwrap();
    let width_2 = network
        .view_table(params_width_2.set_base_path(offset.as_ref()))
        .unwrap();
    insta::assert_snapshot!(width_2, @r###"
    Table {
    X <---- .2 joint (empty) in order
     X <--- .3 joint (empty) in order
      ? <--- (one or more nodes omitted...)
    }
    "###);
}

#[test]
fn limit_width_child() -> eyre::Result<()> {
    const N: usize = 50;
    let mut network = NetworkStrings::from_commands_str(
        "
        add-joint .
        add-joint .
        add-joint .1
        ",
    )?;

    let base = Path::from_str(".1.0").unwrap();
    fill_width_and_depth(&mut network, base.clone(), N);

    // full view is LONG
    let params = TableParams::default().set_base_path(base.as_ref());
    let full = network.view_table(params).unwrap();
    insta::assert_snapshot!(full);

    // shortened view, from beginning column
    let params_width_2 = params.set_max_depth(2).set_max_width(2);
    let width_2 = network.view_table(params_width_2).unwrap();
    insta::assert_snapshot!(width_2, @r###"
    Table {
    XX <--- .1.0 joint (50 children) in order
    X <---- .1.0.0 joint (1 child) in order
     X <--- .1.0.1 joint (empty) in order
      ? <--- (one or more nodes omitted...)
    X <---- .1.0.0.0 joint (1 child hidden) in order
    }
    "###);

    // shortened view, from second column
    let offset_path = Path::from_str(".1.0").unwrap();
    let params_width_2_offset = params_width_2.set_base_path(offset_path.as_ref());
    let width_2_offset = network.view_table(params_width_2_offset).unwrap();
    insta::assert_snapshot!(width_2_offset, @r###"
    Table {
    XX <--- .1.0 joint (50 children) in order
    X <---- .1.0.0 joint (1 child) in order
     X <--- .1.0.1 joint (empty) in order
      ? <--- (one or more nodes omitted...)
    X <---- .1.0.0.0 joint (1 child hidden) in order
    }
    "###);

    Ok(())
}

#[test]
fn node_count() -> eyre::Result<()> {
    let network = NetworkStrings::from_commands_str(
        "
        add-joint .
        add-joint .
        add-joint .0
        add-joint .1
        ",
    )?;

    let params = TableParams::default();
    let limit_none = network.view_table(params).unwrap();
    insta::assert_snapshot!(limit_none, @r###"
    Table {
    X <---- .0 joint (1 child) in order
     X <--- .1 joint (1 child) in order
    X <---- .0.0 joint (empty) in order
     X <--- .1.0 joint (empty) in order
    }
    "###);
    let limit_4 = network.view_table(params.set_max_node_count(4)).unwrap();
    insta::assert_snapshot!(limit_4, @r###"
    Table {
    X <---- .0 joint (1 child) in order
     X <--- .1 joint (1 child) in order
    X <---- .0.0 joint (empty) in order
     X <--- .1.0 joint (empty) in order
    }
    "###);
    let limit_3 = network.view_table(params.set_max_node_count(3)).unwrap();
    insta::assert_snapshot!(limit_3, @r###"
    Table {
    X <---- .0 joint (1 child) in order
     X <--- .1 joint (1 child) in order
    X <---- .0.0 joint (empty) in order
     ? <---- (one or more nodes omitted...)
    }
    "###);
    let limit_2 = network.view_table(params.set_max_node_count(2)).unwrap();
    insta::assert_snapshot!(limit_2, @r###"
    Table {
    X <---- .0 joint (1 child) in order
     X <--- .1 joint (1 child) in order
    ? <----- (one or more nodes omitted...)
    }
    "###);
    let limit_1 = network.view_table(params.set_max_node_count(1)).unwrap();
    insta::assert_snapshot!(limit_1, @r###"
    Table {
    X <--- .0 joint (1 child hidden) in order
     ? <--- (one or more nodes omitted...)
    }
    "###);
    let limit_0 = network.view_table(params.set_max_node_count(0)).unwrap();
    insta::assert_snapshot!(limit_0, @r###"
    Table {
    ? <--- (one or more nodes omitted...)
    }
    "###);

    Ok(())
}

#[test]
fn view_arbitrary_network() {
    arbtest::arbtest(|u| {
        let network: Network<String, String> = Network::arbitrary(u)?;
        println!("{}", network.view_table_default());
        Ok(())
    });
}

mod arbitrary_limit {
    use crate::{
        view::{NodeDetails, TableParams, TableView},
        Network,
    };

    #[derive(Clone, Copy, Debug, arbitrary::Arbitrary)]
    enum NonZeroLimits {
        Depth,
        Width,
        Count,
        DepthWidth,
        DepthCount,
        WidthCount,
        DepthWidthCount,
    }
    #[derive(Clone, Copy, Debug, arbitrary::Arbitrary)]
    enum NonZeroLimitsNoDepth {
        Width,
        Count,
        WidthCount,
    }
    impl From<NonZeroLimitsNoDepth> for NonZeroLimits {
        fn from(value: NonZeroLimitsNoDepth) -> Self {
            use NonZeroLimitsNoDepth as V;
            match value {
                V::Width => Self::Width,
                V::Count => Self::Count,
                V::WidthCount => Self::WidthCount,
            }
        }
    }
    impl NonZeroLimits {
        fn has_depth(self) -> bool {
            match self {
                Self::Depth | Self::DepthWidth | Self::DepthCount | Self::DepthWidthCount => true,
                Self::Width | Self::Count | Self::WidthCount => false,
            }
        }
        fn has_width(self) -> bool {
            match self {
                Self::Width | Self::DepthWidth | Self::WidthCount | Self::DepthWidthCount => true,
                Self::Depth | Self::Count | Self::DepthCount => false,
            }
        }
        fn has_count(self) -> bool {
            match self {
                Self::Count | Self::DepthCount | Self::WidthCount | Self::DepthWidthCount => true,
                Self::Depth | Self::Width | Self::DepthWidth => false,
            }
        }
    }

    fn params_for_view(
        full_view: &TableView,
        u: &mut arbitrary::Unstructured<'_>,
    ) -> arbitrary::Result<TableParams<'static>> {
        let LimitMetrics {
            depth,
            width,
            count,
        } = LimitMetrics::new(full_view);

        let limit = if depth.is_some() {
            u.arbitrary::<NonZeroLimits>()?
        } else {
            u.arbitrary::<NonZeroLimitsNoDepth>()?.into()
        };

        let mut params = TableParams::default();
        if limit.has_depth() {
            let depth = depth.expect("limit with Depth should include depth");
            params = params.set_max_depth(u.int_in_range(0..=depth)?);
        }
        if limit.has_width() {
            params = params.set_max_width(u.int_in_range(0..=width)?);
        }
        if limit.has_count() {
            params = params.set_max_node_count(u.int_in_range(0..=count)?);
        }
        Ok(params)
    }

    struct LimitMetrics {
        depth: Option<u32>,
        width: u32,
        count: u32,
    }
    impl LimitMetrics {
        fn new(full_view: &TableView) -> Self {
            let depth = u32::try_from(full_view.get_rows().len())
                .expect("full_view row count should be within u32::MAX")
                .checked_sub(2);
            let width = full_view.get_max_row_width().saturating_sub(1);
            let count = count_nodes_u32(full_view)
                .expect("full_view node count should be within u32::MAX")
                .saturating_sub(1);
            Self {
                depth,
                width,
                count,
            }
        }
    }

    fn assert_has_abbreviations(view: &TableView, expect_abbreviations: bool, label: &str) {
        if view.get_rows().is_empty() {
            // empty result has no opportunity for tests
            return;
        }
        let has_abbreviations = view.get_rows().iter().any(|row| {
            row.get_cells().iter().any(|cell| {
                let sibling_hidden = cell.get_display_width() == 0;
                let child_hidden = cell
                    .get_node()
                    .map_or(false, NodeDetails::is_joint_children_hidden);
                sibling_hidden || child_hidden
            })
        });
        if expect_abbreviations {
            assert!(has_abbreviations, "{label} should have abbreviations");
        } else {
            assert!(!has_abbreviations, "{label} should NOT have abbreviations");
        }
    }

    fn count_nodes(view: &TableView) -> usize {
        view.get_rows()
            .iter()
            .map(|row| {
                row.get_cells()
                    .iter()
                    .filter(|cell| cell.get_node().is_some())
                    .count()
            })
            .sum()
    }
    fn count_nodes_u32(view: &TableView) -> Option<u32> {
        count_nodes(view).try_into().ok()
    }

    #[test]
    fn view_params() {
        arbtest::arbtest(|u| {
            let network: Network<String, String> = Network::arbitrary(u)?;

            let full_view = network.view_table_default();
            println!("Full: {full_view}");

            assert_has_abbreviations(&full_view, false, "full_view");

            let params = params_for_view(&full_view, u)?;
            dbg!(&params);

            let limited_view = network.view_table(params).unwrap();
            println!("Limited: {limited_view}");

            assert_has_abbreviations(&limited_view, true, "limited_view");

            // TODO fix node-count behavior
            // if let Some(max_node_count) = params.get_max_node_count() {
            //     let nodes_count = count_nodes_u32(&limited_view)
            //         .expect("limit_view should have fewer nodes than u32::MAX");
            //     if params.get_max_width().is_some() || params.get_max_depth().is_some() {
            //         assert!(nodes_count <= max_node_count, "limit_view nodes should respect max, expected {nodes_count} <= {max_node_count}");
            //     } else {
            //         assert_eq!(nodes_count, max_node_count, "limit_view nodes should equal requested max");
            //     }
            // }

            Ok(())
        })
        // TODO verify `view_specific_complex` case is fixed
        // .seed(0xd4c9add400000868)
        ;
    }
}

#[test]
#[ignore = "need to fix node-count behavior"] // TODO
fn view_specific_complex() -> eyre::Result<()> {
    let network = NetworkStrings::from_commands_str(
        "
        set-order-type . shuffle
        add-bucket .
        set-order-type .0 in-order
        add-joint .
        set-weight .1 1988479017
        delete-empty .1
        set-order-type . shuffle
        set-weight .0 647436531
        set-weight .0 3135499917
        add-joint .
        add-joint .1
        add-joint .1.0
        set-order-type .1 in-order
        add-joint .1.0.0
        set-weight .1 110430342
        add-bucket .1.0.0.0
        add-joint .1.0.0.0
        add-joint .1
        add-joint .1.1
        add-joint .
        set-weight .1.1.0 2412235562
        add-joint .1.0
        add-joint .1.0.1
        add-bucket .1.0.0.0
        add-bucket .2
        set-weight .1.0.1 2297929128
        set-order-type .2 in-order
        add-joint .1.0
        add-joint .1.1.0
        set-order-type .1.0.0 in-order
        ",
    )?;
    insta::assert_snapshot!(network.view_table_default(), @r###"
    Table {
    X <---------- .0 x3135499917 bucket (empty) in order
     XXXXXX <---- .1 x110430342 joint (2 children) in order
           X <--- .2 x1 joint (1 child) in order
     XXXXX <----- .1.0 joint (3 children) in order
          X <---- .1.1 joint (1 child) in order
           X <--- .2.0 bucket (empty) in order
     XXX <------- .1.0.0 x1 joint (1 child) in order
        X <------ .1.0.1 x2297929128 joint (1 child) in order
         X <----- .1.0.2 x1 joint (empty) in order
          X <---- .1.1.0 x2412235562 joint (1 child) in order
     XXX <------- .1.0.0.0 joint (3 children) in order
        X <------ .1.0.1.0 joint (empty) in order
          X <---- .1.1.0.0 joint (empty) in order
     X <--------- .1.0.0.0.0 bucket (empty) in order
      X <-------- .1.0.0.0.1 joint (empty) in order
       X <------- .1.0.0.0.2 bucket (empty) in order
    }
    "###);

    let params = TableParams::default().set_max_node_count(11);
    insta::assert_snapshot!(network.view_table(params).unwrap(), @r###"
    Table {
    X <-------- .0 x3135499917 bucket (empty) in order
     XXXX <---- .1 x110430342 joint (2 children) in order
         X <--- .2 x1 joint (1 child) in order
     XXX <----- .1.0 joint (3 children) in order
        X <---- .1.1 joint (1 child) in order
         X <--- .2.0 bucket (empty) in order
     X <------- .1.0.0 x1 joint (1 child) in order
      X <------ .1.0.1 x2297929128 joint (1 child) in order
       X <----- .1.0.2 x1 joint (empty) in order
        X <---- .1.1.0 x2412235562 joint (1 child) in order
     X <------- .1.0.0.0 joint (3 children) in order
      ? <------ (one or more nodes omitted...)
    }
    "###);

    Ok(())
}
