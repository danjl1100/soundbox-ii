// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use crate::{path::Path, view::TableParamsOwned, Network};
use std::str::FromStr as _;

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

    let params = TableParamsOwned::default();
    let params = params.as_ref();

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

fn arbitrary_pattern() -> Network<String, String> {
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
    let network = arbitrary_pattern();

    let params = TableParamsOwned::default();
    let params = params.as_ref();

    let root_max = network.view_table(params).unwrap();
    let root_depth_2 = network.view_table(params.max_depth(2)).unwrap();
    let root_depth_1 = network.view_table(params.max_depth(1)).unwrap();
    let root_depth_0 = network.view_table(params.max_depth(0)).unwrap();
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
}

#[test]
fn table_depth_child_right() {
    let network = arbitrary_pattern();

    let params = TableParamsOwned::default();
    let path = Path::from_str(".4").unwrap();
    let params = params.as_ref().base_path(path.as_ref());

    let right_max = network.view_table(params).unwrap();
    let right_depth_2 = network.view_table(params.max_depth(2)).unwrap();
    let right_depth_1 = network.view_table(params.max_depth(1)).unwrap();
    let right_depth_0 = network.view_table(params.max_depth(0)).unwrap();
    assert_eq!(right_max, right_depth_2);
    assert_eq!(right_max, right_depth_1);
    insta::assert_snapshot!(right_depth_1, @r###"
    Table {
    X <----- .4.0 bucket (empty) in order
     XX <--- .4.1 joint (2 children) in order
     X <---- .4.1.0 bucket (empty) in order
      X <--- .4.1.1 bucket (empty) in order
    }
    "###);
    insta::assert_snapshot!(right_depth_0, @r###"
    Table {
    X <---- .4.0 bucket (empty) in order
     X <--- .4.1 joint (2 children hidden) in order
    }
    "###);
}
