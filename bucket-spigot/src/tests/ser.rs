// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use crate::{clap::ArgBounds, order::OrderType, path::Path, ModifyCmd, Network};

fn into_cmds<T, U>(network: Network<T, U>) -> Vec<ModifyCmd<T, U>>
where
    T: Clone + serde::Serialize + serde::de::DeserializeOwned + std::fmt::Debug,
    U: Clone + serde::Serialize + serde::de::DeserializeOwned + std::fmt::Debug,
    ModifyCmd<T, U>: PartialEq,
{
    let cmds = network.serialize_collect();

    let cmds_serde = {
        // Structs are needed to test the `serde` portion

        #[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
        struct AsNetwork<T, U>(
            #[serde(serialize_with = "Network::serialize_into_modify_commands")]
            #[serde(deserialize_with = "Network::deserialize_from_modify_commands")]
            Network<T, U>,
        )
        where
            T: serde::Serialize + serde::de::DeserializeOwned,
            U: serde::Serialize + serde::de::DeserializeOwned;

        #[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
        struct AsModifyCmds<T, U>(Vec<ModifyCmd<T, U>>);

        let wrapped = AsNetwork(network);
        let json = serde_json::to_string(&wrapped).unwrap();
        let AsModifyCmds(cmds) = serde_json::from_str(&json).unwrap();
        cmds
    };

    assert_eq!(cmds, cmds_serde);

    cmds
}

fn cmds_script<T, U>(cmds: &[ModifyCmd<T, U>]) -> String
where
    T: ArgBounds + PartialEq,
    U: ArgBounds + PartialEq,
{
    use std::fmt::Write;

    let mut script = String::new();
    for cmd in cmds {
        let cmd_string = cmd.display_as_cmd_verified();

        writeln!(&mut script, "{cmd_string}").unwrap();
    }
    println!("----- SCRIPT -----\n{script}------------------");
    script
}

fn assert_rebuilds<T, U>(cmds: Vec<crate::ModifyCmd<T, U>>, expected: &Network<T, U>)
where
    T: ArgBounds + PartialEq,
    U: ArgBounds + PartialEq,
{
    let mut network_rebuilt = Network::default();
    let cmds_is_empty = cmds.is_empty();
    let cmds_summary = {
        use std::fmt::Write as _;
        cmds.iter().fold(String::new(), |mut s, cmd| {
            write!(s, "\n\t{}", cmd.display_as_cmd_verified()).expect("infallible");
            s
        })
    };
    for cmd in cmds {
        network_rebuilt.modify(cmd).unwrap();
    }
    if false {
        // TODO remove diabolical test-of-tests
        if cmds_is_empty {
            network_rebuilt
                .modify(ModifyCmd::AddBucket {
                    parent: Path::empty(),
                })
                .unwrap();
        }
    }

    let view_expected = expected.view_table_default();
    let view_rebuilt = network_rebuilt.view_table_default();

    // TODO verify this check is sufficient (e.g. observability, table view is not accidentally too opaque)
    assert_eq!(view_expected, view_rebuilt, "{cmds_summary}");
}

fn check_rebuilds_script(script: &str, extra_cmds: Vec<ModifyCmd<String, String>>) {
    let (mut network, _log) = Network::new_strings_build_from_script(script);
    for cmd in extra_cmds {
        network.modify(cmd).unwrap();
    }

    network.check_ser(|_| ());
}
impl Network<String, String> {
    fn check_ser(&self, inspect_fn: impl FnOnce(&[ModifyCmd<String, String>])) {
        let cmds = into_cmds(self.clone());
        inspect_fn(&cmds);
        assert_rebuilds(cmds.clone(), self);
    }
}

#[test]
fn empty() {
    Network::new_strings().check_ser(|cmds| {
        insta::assert_snapshot!(cmds_script(cmds), @"");
    });
}

#[test]
fn nodes_shallow() {
    let (network, _log) = Network::new_strings_build_from_script(
        "
        modify add-bucket .
        modify add-bucket .
        modify add-bucket .
        modify add-bucket .
        ",
    );
    network.check_ser(|cmds| {
        insta::assert_snapshot!(cmds_script(cmds), @r###"
        add-bucket .
        add-bucket .
        add-bucket .
        add-bucket .
        "###);
    });
}

#[test]
fn nodes_narrow() {
    let (network, _log) = Network::new_strings_build_from_script(
        "
        modify add-joint .
        modify add-joint .0
        modify add-joint .0.0
        ",
    );
    network.check_ser(|cmds| {
        insta::assert_snapshot!(cmds_script(cmds), @r###"
        add-joint .
        add-joint .0
        add-joint .0.0
        "###);
    });
}

#[test]
fn node_placement() {
    let (network, _log) = Network::new_strings_build_from_script(
        "
        modify add-bucket .
        modify add-joint .

        modify add-bucket .1
        modify add-bucket .1
        modify add-joint .1
        modify add-bucket .1

        modify add-bucket .1.2
        ",
    );
    network.check_ser(|cmds| {
        insta::assert_snapshot!(cmds_script(cmds), @r###"
        add-bucket .
        add-joint .
        add-bucket .1
        add-bucket .1
        add-joint .1
        add-bucket .1.2
        add-bucket .1
        "###);
    });
}

#[test]
fn node_order_type() {
    for new_order_type in OrderType::iter_all() {
        let set_order_type = ModifyCmd::SetOrderType {
            path: ".0".parse().unwrap(),
            new_order_type,
        };
        check_rebuilds_script("modify add-joint .", vec![set_order_type]);
    }
}

#[test]
fn node_weight() {
    for new_weight in [1, 5, 10, 100] {
        let set_weight = ModifyCmd::SetWeight {
            path: ".0".parse().unwrap(),
            new_weight,
        };
        check_rebuilds_script("modify add-joint .", vec![set_weight]);
    }
}

mod arb_network {
    use crate::{order::OrderType, path::Path, view::TableParams, ModifyCmd, Network};

    #[derive(arbtest::arbitrary::Arbitrary)]
    enum OrderTypeSeed {
        InOrder,
        Random,
        Shuffle,
    }
    impl From<OrderType> for OrderTypeSeed {
        fn from(value: OrderType) -> Self {
            use OrderType as Other;
            match value {
                Other::InOrder => Self::InOrder,
                Other::Random => Self::Random,
                Other::Shuffle => Self::Shuffle,
            }
        }
    }
    impl From<OrderTypeSeed> for OrderType {
        fn from(value: OrderTypeSeed) -> Self {
            use OrderTypeSeed as Other;
            match value {
                Other::InOrder => Self::InOrder,
                Other::Random => Self::Random,
                Other::Shuffle => Self::Shuffle,
            }
        }
    }
    #[derive(arbtest::arbitrary::Arbitrary)]
    enum ModifyCmdSeed<U> {
        AddBucket,
        AddJoint,
        // DeleteEmpty,
        //
        // TODO add FillBucket
        // FillBucket { new_contents: Vec<T> },
        SetFilters { new_filters: Vec<U> },
        SetWeight { new_weight: u32 },
        SetOrderType { new_order_type: OrderTypeSeed },
    }
    impl<T, U> From<ModifyCmd<T, U>> for (Path, ModifyCmdSeed<U>) {
        #[rustfmt::skip]
        fn from(value: ModifyCmd<T, U>) -> Self {
            use ModifyCmd as Cmd;
            use ModifyCmdSeed as Seed;
            match value {
                Cmd::AddBucket { parent } => (parent, Seed::AddBucket),
                Cmd::AddJoint { parent } => (parent, Seed::AddJoint),
                Cmd::DeleteEmpty { path: _ } => todo!(), // (path, Seed::DeleteEmpty),
                Cmd::FillBucket { bucket: _, new_contents: _, } => todo!(), // TODO reinstate: (bucket, Seed::FillBucket { new_contents }),
                Cmd::SetFilters { path, new_filters } => (path, Seed::SetFilters { new_filters }),
                Cmd::SetWeight { path, new_weight } => (path, Seed::SetWeight { new_weight }),
                Cmd::SetOrderType { path, new_order_type, } => (path, Seed::SetOrderType { new_order_type: new_order_type.into() }),
            }
        }
    }
    impl<T, U> From<(Path, ModifyCmdSeed<U>)> for ModifyCmd<T, U> {
        #[rustfmt::skip]
        fn from(value: (Path, ModifyCmdSeed<U>)) -> Self {
            use ModifyCmd as Cmd;
            use ModifyCmdSeed as Seed;
            match value {
                (parent, Seed::AddBucket) => Cmd::AddBucket { parent },
                (parent, Seed::AddJoint) => Cmd::AddJoint { parent },
                // (path, Seed::DeleteEmpty) => Cmd::DeleteEmpty { path },
                // TODO reinstate: (bucket, Seed::FillBucket { new_contents }) => Cmd::FillBucket { bucket, new_contents, },
                (path, Seed::SetFilters { new_filters }) => Cmd::SetFilters { path, new_filters },
                (path, Seed::SetWeight { new_weight }) => Cmd::SetWeight { path, new_weight },
                (path, Seed::SetOrderType { new_order_type }) => Cmd::SetOrderType { path, new_order_type: new_order_type.into() },
            }
        }
    }

    impl<'a, T, U> arbitrary::Arbitrary<'a> for Network<T, U>
    where
        T: crate::clap::ArgBounds + arbitrary::Arbitrary<'a>,
        U: crate::clap::ArgBounds + arbitrary::Arbitrary<'a>,
    {
        fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
            use ModifyCmdSeed as Seed;

            let mut network = Self::default();

            let mut node_paths = vec![Path::empty()];
            let mut bucket_paths = vec![];
            let mut joint_paths = vec![Path::empty()];

            for _ in 0..u.arbitrary_len::<ModifyCmdSeed<U>>()? {
                let seed: ModifyCmdSeed<U> = u.arbitrary()?;
                let path_options = match &seed {
                    // only joints
                    Seed::AddBucket | Seed::AddJoint => &joint_paths,
                    // // TODO creating network should not invoke Delete
                    // Seed::DeleteEmpty => todo!(),
                    // any node
                    // TODO reinstate: Seed::FillBucket { .. } => &bucket_paths,
                    Seed::SetOrderType { .. } => &node_paths,
                    // exclude root
                    Seed::SetFilters { .. } | Seed::SetWeight { .. } => &node_paths[1..],
                };
                let path = u.choose(path_options)?;

                let child_of_current = {
                    // TODO this seems expensive, for just counting the number of children
                    // (but it's only TEST code... right?   for now?)
                    let mut child = path.clone();
                    child.push(0);
                    child
                };
                let table_view = network
                    .view_table(
                        TableParams::default()
                            .set_base_path(child_of_current.as_ref())
                            .set_max_depth(0),
                    )
                    .ok();

                // TODO remove redundant calculation (dedicated method is _so much better_)
                let len_of_dest_2: usize = table_view.as_ref().map_or(0, |table_view| {
                    table_view
                        .get_max_row_width()
                        .try_into()
                        .expect("u32 should fit in usize for Arbitrary table dimensions")
                });
                let len_of_dest = network
                    .count_child_nodes_of(path.clone())
                    .expect("current path should be valid");
                assert_eq!(len_of_dest.unwrap_or(0), len_of_dest_2);

                let get_new_path = || {
                    let mut new = path.clone();
                    new.push(len_of_dest.expect("only add to joint"));

                    // DEBUG
                    let table_view_all = network
                        .view_table(TableParams::default())
                        .expect("impl Arbitrary for Network should create valid views");
                    println!("FULL {table_view_all}");
                    if let Some(table_view) = &table_view {
                        println!("@ {path} {table_view}");
                    } else {
                        println!("@ {path} - EMPTY");
                    }
                    // dbg!(len_of_dest, &new);

                    assert!(!bucket_paths.contains(&new));
                    assert!(!joint_paths.contains(&new));
                    assert!(!node_paths.contains(&new));

                    new
                };

                let path_clone = path.clone();

                // update path lists
                match &seed {
                    Seed::AddBucket => {
                        let new_path = get_new_path();

                        bucket_paths.push(new_path.clone());
                        node_paths.push(new_path);
                    }
                    Seed::AddJoint => {
                        let new_path = get_new_path();

                        joint_paths.push(new_path.clone());
                        node_paths.push(new_path);
                    }
                    // Seed::DeleteEmpty
                    // TODO reinstate: Seed::FillBucket { .. }
                    Seed::SetFilters { .. }
                    | Seed::SetWeight { .. }
                    | Seed::SetOrderType { .. } => {}
                }

                let cmd = ModifyCmd::from((path_clone, seed));
                let cmd_str = cmd.display_as_cmd().to_string();
                println!("-> {cmd_str}");
                if let Err(e) = network.modify(cmd) {
                    panic!("impl Arbitrary for Network should only execute valid commands: {e} \nModifyCmd: {cmd_str}");
                }
            }

            Ok(network)
        }
        // TODO remove, just promise to never construct `Vec<Network<_, _>>`
        // fn size_hint(depth: usize) -> (usize, Option<usize>) {
        //     todo!()
        // }
    }
}

// fuzz the input ModifyCmds
#[test]
fn check_arbitrary_network() {
    arbtest::arbtest(|u| {
        let network: Network<String, String> = u.arbitrary()?;
        network.check_ser(|_| ());
        Ok(())
    });
}
