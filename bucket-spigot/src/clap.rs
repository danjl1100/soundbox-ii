// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! [`clap`] compatible versions of types

use crate::path::Path;

// re-export `clap`
#[allow(clippy::module_name_repetitions, unused)]
pub use ::clap as clap_crate;
use std::str::FromStr;

/// Generic bounds required for all [`ModifyCmd`] type parameters
pub trait ArgBounds:
    FromStr<Err: std::error::Error + Send + Sync + 'static>
    + Clone
    + std::fmt::Debug
    + Send
    + Sync
    + 'static
{
}
impl<T> ArgBounds for T
where
    Self: FromStr + Clone + std::fmt::Debug + Send + Sync + 'static,
    Self::Err: std::error::Error + Send + Sync + 'static,
{
}

/// Low-level Control commands for VLC (correspond to a single API call)
#[derive(Clone, clap::Subcommand, Debug, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum ModifyCmd<T, U>
where
    T: ArgBounds,
    U: ArgBounds,
{
    /// Add a new bucket
    AddBucket {
        /// Parent path for the new bucket
        parent: Path,
    },
    /// Add a new joint
    AddJoint {
        /// Parent path for the new joint
        parent: Path,
    },
    /// Delete a node (bucket/joint) that is empty
    DeleteEmpty {
        /// Path of the node (bucket/joint) to delete
        path: Path,
    },
    /// Set the contents of the specified bucket
    ///
    /// Removes the bucket from the "needing fill" list (if present)
    FillBucket {
        /// Path of the bucket to fill
        bucket: Path,
        /// Items for the bucket
        new_contents: Vec<T>,
    },
    /// Set the filters on a joint or bucket
    SetFilters {
        /// Path for the existing joint or bucket
        path: Path,
        /// List of filters to set
        new_filters: Vec<U>,
    },
    /// Set the weight on a joint or bucket
    SetWeight {
        /// Path for the existing joint or bucket
        path: Path,
        /// Weight value (relative to other weights on sibling nodes)
        new_weight: u32,
    },
    /// Set the ordering type for the joint or bucket
    SetOrderType {
        /// Path for the existing joint or bucket
        path: Path,
        /// Order type (how to select from immediate child nodes or items)
        new_order_type: OrderType,
    },
}
/// Ordering scheme for child nodes of a joint, or child items of a bucket
///
/// NOTE: Separate from [`crate::order::OrderType`] to emphasize `clap` as a public (string) interface
#[derive(Clone, clap::ValueEnum, Debug, serde::Serialize, serde::Deserialize)]
pub enum OrderType {
    /// Selects each child in turn, repeating each according to the weights
    InOrder,
    /// Selects a random (weighted) child
    Random,
    /// Selects from a randomized order of the children
    /// NOTE: For N total child-weight choices, the result is the shuffled version of
    /// [`InOrder`](`Self::InOrder`)
    Shuffle,
}
impl<T, U> From<ModifyCmd<T, U>> for crate::ModifyCmd<T, U>
where
    T: ArgBounds,
    U: ArgBounds,
{
    fn from(value: ModifyCmd<T, U>) -> Self {
        match value {
            ModifyCmd::AddBucket { parent } => Self::AddBucket { parent },
            ModifyCmd::AddJoint { parent } => Self::AddJoint { parent },
            ModifyCmd::DeleteEmpty { path } => Self::DeleteEmpty { path },
            ModifyCmd::FillBucket {
                bucket,
                new_contents,
            } => Self::FillBucket {
                bucket,
                new_contents,
            },
            ModifyCmd::SetFilters { path, new_filters } => Self::SetFilters { path, new_filters },
            ModifyCmd::SetWeight { path, new_weight } => Self::SetWeight { path, new_weight },
            ModifyCmd::SetOrderType {
                path,
                new_order_type,
            } => Self::SetOrderType {
                path,
                new_order_type: new_order_type.into(),
            },
        }
    }
}
impl From<OrderType> for crate::order::OrderType {
    fn from(value: OrderType) -> Self {
        match value {
            OrderType::InOrder => Self::InOrder,
            OrderType::Shuffle => Self::Shuffle,
            OrderType::Random => Self::Random,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ModifyCmd;
    use clap::Parser as _;

    #[derive(clap::Parser, serde::Serialize)]
    struct TestCli {
        #[clap(subcommand)]
        modify_cmd: ModifyCmd<String, String>,
    }

    fn parse_cli(args: &[&'static str]) -> Result<crate::ModifyCmd<String, String>, String> {
        let args = std::iter::once("_").chain(args.iter().copied());
        TestCli::try_parse_from(args)
            .map(|TestCli { modify_cmd }| modify_cmd.into())
            .map_err(|err| err.to_string())
    }

    #[test]
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
    #[test]
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
    #[test]
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
    #[test]
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
    #[test]
    fn set_weight() {
        insta::assert_ron_snapshot!(parse_cli(&["set-weight", ".1.2.3.4", "50"]), @r###"
        Ok(SetWeight(
          path: ".1.2.3.4",
          new_weight: 50,
        ))
        "###);
    }
}
