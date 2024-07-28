// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! [`clap`] compatible versions of types

use crate::path::Path;

// re-export `clap`
#[allow(clippy::module_name_repetitions, unused)]
pub use ::clap as clap_crate;
use std::str::FromStr;

/// Low-level Control commands for VLC (correspond to a single API call)
#[derive(Clone, clap::Subcommand, Debug, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum ModifyCmd<T, U>
where
    T: FromStr + Clone + std::fmt::Debug + Send + Sync + 'static,
    U: FromStr + Clone + std::fmt::Debug + Send + Sync + 'static,
    T::Err: std::error::Error + Send + Sync + 'static,
    U::Err: std::error::Error + Send + Sync + 'static,
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
    /// Set the contents of the specified bucket
    ///
    /// Removes the bucket from the "needing fill" list (if present)
    FillBucket {
        /// Path of the bucket to fill
        bucket: Path,
        /// Items for the bucket
        new_contents: Vec<T>,
    },
    /// Set the filters on a joint
    SetJointFilters {
        /// Path for the existing joint
        joint: Path,
        /// List of filters to set on the joint
        new_filters: Vec<U>,
    },
}
impl<T, U> From<ModifyCmd<T, U>> for crate::ModifyCmd<T, U>
where
    T: FromStr + Clone + std::fmt::Debug + Send + Sync + 'static,
    U: FromStr + Clone + std::fmt::Debug + Send + Sync + 'static,
    T::Err: std::error::Error + Send + Sync + 'static,
    U::Err: std::error::Error + Send + Sync + 'static,
{
    fn from(value: ModifyCmd<T, U>) -> Self {
        match value {
            ModifyCmd::AddBucket { parent } => Self::AddBucket { parent },
            ModifyCmd::AddJoint { parent } => Self::AddJoint { parent },
            ModifyCmd::FillBucket {
                bucket,
                new_contents,
            } => Self::FillBucket {
                bucket,
                new_contents,
            },
            ModifyCmd::SetJointFilters { joint, new_filters } => {
                Self::SetJointFilters { joint, new_filters }
            }
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
    fn set_joint_filters() {
        insta::assert_ron_snapshot!(parse_cli(&["set-joint-filters", ".1.2", "a", "b", "foo"]), @r###"
        Ok(SetJointFilters(
          joint: ".1.2",
          new_filters: [
            "a",
            "b",
            "foo",
          ],
        ))
        "###);
    }
}
