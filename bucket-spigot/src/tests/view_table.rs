// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use crate::{
    path::{Path, PathRef},
    view::TableParams,
    Network,
};
use std::str::FromStr as _;

#[test]
fn empty() {
    let network = Network::new_strings();
    let table = network.view_table_default();
    insta::assert_snapshot!(table, @r###"
    Table {
    }
    "###);
}

#[test]
fn table_weights() {
    let mut network = Network::new_strings();
    let log = network.run_script(
        "
        modify add-bucket .
        modify fill-bucket .0 abc def ghi jkl
        modify add-joint .
        modify add-joint .1
        modify add-bucket .1
        modify fill-bucket .1.1 qrs tuv wxyz

        modify add-joint .1.0
        modify add-bucket .1.0
        modify fill-bucket .1.0.1 1 2 3 4
        modify add-bucket .1.0
        modify fill-bucket .1.0.2 5 6 7 8 9

        topology
        ",
    );
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
}

fn arbitrary_pattern1() -> Network<String, String> {
    let mut network = Network::new_strings();
    network.run_script(
        "
        modify add-joint .
        modify add-bucket .
        modify add-bucket .
        modify add-bucket .
        modify add-joint .

        modify add-joint .0
        modify add-bucket .0

        modify add-bucket .0.0
        modify add-bucket .0.0

        modify add-bucket .4
        modify add-joint .4

        modify add-bucket .4.1
        modify add-bucket .4.1

        modify set-weight .0 0
        modify set-weight .0.1 50
        modify set-weight .1 2
        modify set-weight .2 3
        modify set-weight .3 4
        ",
    );
    network
}

#[test]
fn table_depth_root() {
    let network = arbitrary_pattern1();

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
}

fn view_path<T, U>(network: &Network<T, U>, path_str: &str) -> String {
    let path = Path::from_str(path_str).unwrap();
    let params = TableParams::default().set_base_path(path.as_ref());
    network.view_table(params).unwrap().to_string()
}

#[test]
fn unique_weights() {
    let mut network = Network::new_strings();
    network.run_script(
        "
        modify add-joint .
        modify add-joint .
        modify add-joint .
        modify add-joint .
        modify add-joint .

        modify add-joint .0
        modify add-joint .0
        modify add-joint .0
        modify add-joint .0
        modify add-joint .0

        modify set-weight .0 1
        modify set-weight .1 2
        modify set-weight .2 3
        modify set-weight .3 4
        modify set-weight .4 5
        modify set-weight .0.0 6
        modify set-weight .0.1 7
        modify set-weight .0.2 8
        modify set-weight .0.3 9
        modify set-weight .0.4 10
        ",
    );
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
}

#[test]
fn table_depth_child_right() {
    let network = arbitrary_pattern1();

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
}

#[test]
fn table_view_bucket() {
    let mut network = Network::new_strings();
    network.run_script(
        "
        modify add-joint .
        modify add-bucket .
        modify add-bucket .0
        ",
    );
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
}

/// Add `count` child joints to specified node
fn fill_width_at<T, U>(network: &mut Network<T, U>, parent: PathRef<'_>, count: usize) {
    for _ in 0..count {
        network
            .modify(crate::ModifyCmd::AddJoint {
                parent: parent.clone_inner(),
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
    let mut network = Network::new_strings();

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
    }
    "###);
}

#[test]
fn limit_width_child() {
    const N: usize = 50;
    let mut network = Network::new_strings();
    network.run_script(
        "
        modify add-joint .
        modify add-joint .
        modify add-joint .1
        ",
    );

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
}
