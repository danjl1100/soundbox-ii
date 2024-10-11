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
impl<T, U> Network<T, U>
where
    T: ArgBounds + PartialEq + serde::Serialize + serde::de::DeserializeOwned,
    U: ArgBounds + PartialEq + serde::Serialize + serde::de::DeserializeOwned,
{
    fn check_ser(&self, inspect_fn: impl FnOnce(&[ModifyCmd<T, U>])) {
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

// fuzz the input ModifyCmds
#[test]
fn check_arbitrary_network() {
    arbtest::arbtest(|u| {
        let network: Network<_, String> = Network::arbitrary_no_items(u)?;

        network.check_ser(|_| ());
        Ok(())
    });
}
