// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use crate::{
    clap::{ArgBounds, ModifyCmd, OrderType},
    path::Path,
};
use clap::{Parser as _, ValueEnum as _};

/// Verify that a test block runs for all variants of the type
macro_rules! test_exhaustive {
    (
        for $ty:ty ,
        $($pattern:pat => $block:block)+
    ) => {
        // verify patterns are exhaustive
        let _ = |value: $ty| {
            match value {
                $($pattern => {})+
            }
        };
        // execute blocks
        $($block)+
    };
}

#[test]
#[should_panic = "executed the expected block"]
fn macro_runs_block() {
    test_exhaustive! {
        for bool,
        true => {}
        false => { panic!("executed the expected block") }
    }
}

#[derive(clap::Parser, serde::Serialize)]
#[clap(no_binary_name = true)]
struct TestCli {
    #[clap(subcommand)]
    modify_cmd: ModifyCmd<String, String>,
}

fn parse_cli(args: &[&'static str]) -> Result<crate::ModifyCmd<String, String>, String> {
    let args = args.iter().copied();
    TestCli::try_parse_from(args)
        .map(|TestCli { modify_cmd }| modify_cmd.into())
        .map_err(|err| err.to_string())
}

fn add_bucket() {
    insta::assert_ron_snapshot!(parse_cli(&["add-bucket", "."]), @r###"
    Ok(AddBucket(
      parent: ".",
    ))
    "###);
    insta::assert_ron_snapshot!(parse_cli(&["add-bucket", ".1.2.3.4"]), @r###"
    Ok(AddBucket(
      parent: ".1.2.3.4",
    ))
    "###);
}
fn add_joint() {
    insta::assert_ron_snapshot!(parse_cli(&["add-joint", "."]), @r###"
    Ok(AddJoint(
      parent: ".",
    ))
    "###);
    insta::assert_ron_snapshot!(parse_cli(&["add-joint", ".1.2.3.4"]), @r###"
    Ok(AddJoint(
      parent: ".1.2.3.4",
    ))
    "###);
}
fn delete_empty() {
    insta::assert_ron_snapshot!(parse_cli(&["delete-empty", ".5.6.7.8"]), @r###"
    Ok(DeleteEmpty(
      path: ".5.6.7.8",
    ))
    "###);
}
fn fill_bucket() {
    insta::assert_ron_snapshot!(parse_cli(&["fill-bucket", ".1.2.3.4", "a", "b", "foo"]), @r###"
    Ok(FillBucket(
      bucket: ".1.2.3.4",
      new_contents: [
        "a",
        "b",
        "foo",
      ],
    ))
    "###);
}
fn set_filters() {
    insta::assert_ron_snapshot!(parse_cli(&["set-filters", ".1.2", "a", "b", "foo"]), @r###"
    Ok(SetFilters(
      path: ".1.2",
      new_filters: [
        "a",
        "b",
        "foo",
      ],
    ))
    "###);
}
fn set_weight() {
    insta::assert_ron_snapshot!(parse_cli(&["set-weight", ".1.2.3.4", "50"]), @r###"
    Ok(SetWeight(
      path: ".1.2.3.4",
      new_weight: 50,
    ))
    "###);
}
fn set_order_type() {
    insta::assert_ron_snapshot!(parse_cli(&["set-order-type", ".5.6.7.8", "in-order"]), @r###"
    Ok(SetOrderType(
      path: ".5.6.7.8",
      new_order_type: InOrder,
    ))
    "###);
    insta::assert_ron_snapshot!(parse_cli(&["set-order-type", ".5.6.7.8", "random"]), @r###"
    Ok(SetOrderType(
      path: ".5.6.7.8",
      new_order_type: Random,
    ))
    "###);
    insta::assert_ron_snapshot!(parse_cli(&["set-order-type", ".5.6.7.8", "shuffle"]),@r###"
    Ok(SetOrderType(
      path: ".5.6.7.8",
      new_order_type: Shuffle,
    ))
    "###);
}
#[test]
fn parse_cli_exhaustive() {
    test_exhaustive! {
        for ModifyCmd<String, String>,
        ModifyCmd::AddBucket { .. } => { add_bucket(); }
        ModifyCmd::AddJoint { .. } => { add_joint(); }
        ModifyCmd::DeleteEmpty { .. } => { delete_empty(); }
        ModifyCmd::FillBucket { .. } => { fill_bucket(); }
        ModifyCmd::SetFilters { .. } => { set_filters(); }
        ModifyCmd::SetWeight { .. } => { set_weight(); }
        ModifyCmd::SetOrderType { .. } => { set_order_type(); }
    }
}

impl<T, U> crate::ModifyCmd<T, U>
where
    T: ArgBounds + PartialEq,
    U: ArgBounds + PartialEq,
{
    pub(crate) fn display_as_cmd_verified(&self) -> String {
        use clap::Parser as _;

        #[derive(clap::Parser)]
        #[clap(no_binary_name = true)]
        struct FakeCmd<T, U>
        where
            T: ArgBounds,
            U: ArgBounds,
        {
            #[clap(subcommand)]
            cmd: crate::clap::ModifyCmd<T, U>,
        }
        let cmd_string = self.display_as_cmd().to_string();
        {
            // verify equivalent re-parse
            let reparsed_cmd = {
                let FakeCmd { cmd: reparsed_cmd } =
                    FakeCmd::try_parse_from(arg_util::ArgSplit::split_into_owned(&cmd_string))
                        .unwrap_or_else(|err| {
                            eprintln!("{err}");
                            panic!("ModifyCmd::display_as_cmd should produce a valid subcommand")
                        });
                crate::ModifyCmd::from(reparsed_cmd)
            };
            assert_eq!(self, &reparsed_cmd);
        }
        cmd_string
    }
}

#[test]
fn clap_display_roundtrip() {
    type CrateModifyCmd = crate::ModifyCmd<String, String>;
    let path1: Path = ".1.2.3.4".parse().unwrap();

    test_exhaustive!(for CrateModifyCmd,
        CrateModifyCmd::AddBucket { .. } => {
            CrateModifyCmd::AddBucket {
                parent: path1.clone(),
            }
            .display_as_cmd_verified();
        }
        CrateModifyCmd::AddJoint { .. } => {
            CrateModifyCmd::AddJoint {
                parent: path1.clone(),
            }
            .display_as_cmd_verified();
        }
        CrateModifyCmd::DeleteEmpty { .. } => {
            CrateModifyCmd::DeleteEmpty {
                path: path1.clone(),
            }
            .display_as_cmd_verified();
        }
        CrateModifyCmd::FillBucket { .. } => {
            CrateModifyCmd::FillBucket {
                bucket: path1.clone(),
                new_contents: ["a", "bcd", "efgh"]
                    .into_iter()
                    .map(str::to_owned)
                    .collect(),
            }
            .display_as_cmd_verified();
        }
        CrateModifyCmd::SetFilters { .. } => {
            CrateModifyCmd::SetFilters {
                path: path1.clone(),
                new_filters: ["this one", "has some", "spaces     !"]
                    .into_iter()
                    .map(str::to_owned)
                    .collect(),
            }
            .display_as_cmd_verified();
        }
        CrateModifyCmd::SetWeight { .. } => {
            CrateModifyCmd::SetWeight {
                path: path1.clone(),
                new_weight: 25,
            }
            .display_as_cmd_verified();
        }
        CrateModifyCmd::SetOrderType { .. } => {
            for new_order_type in OrderType::value_variants() {
                CrateModifyCmd::SetOrderType {
                    path: path1.clone(),
                    new_order_type: (*new_order_type).into(),
                }
                .display_as_cmd_verified();
            }
        }
    );
}
