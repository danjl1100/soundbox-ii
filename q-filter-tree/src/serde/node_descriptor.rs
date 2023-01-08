// Copyright (C) 2021-2023  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use std::str::FromStr;

use serde::{de::Visitor, Deserialize};

use crate::id::{NodeIdTyped, NodePathTyped};

shared::wrapper_enum! {
    /// Sum type of a [`NodePathTyped`] or [`NodeIdTyped`], for use in loosely interpreting user input
    #[allow(missing_docs)]
    #[derive(Debug)]
    pub enum NodeDescriptor {
        Path(NodePathTyped),
        Id(NodeIdTyped),
    }
}
// TODO: is this needed?  or even useful? deleteme
// impl std::fmt::Display for NodeDescriptor {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         match self {
//             Self::Path(node_path) => write!(f, "{node_path}"),
//             Self::Id(node_id) => write!(f, "{node_id}"),
//         }
//     }
// }
//
// impl Serialize for NodeDescriptor {
//     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//     where
//         S: serde::Serializer,
//     {
//         match self {
//             Self::Path(node_path) => node_path.serialize(serializer),
//             Self::Id(node_id) => node_id.serialize(serializer),
//         }
//     }
// }

impl<'de> Deserialize<'de> for NodeDescriptor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_str(NodeDescriptorVisitor)
    }
}
struct NodeDescriptorVisitor;
impl<'de> Visitor<'de> for NodeDescriptorVisitor {
    type Value = NodeDescriptor;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        let path_description = NodePathTyped::SERIALIZED_DESCRIPTION;
        let id_addendum = NodeIdTyped::SERIALIZED_ADDENDUM;
        formatter.write_fmt(format_args!(
            "either path ({path_description}) or id ({path_description}, followed by {id_addendum}"
        ))
    }
    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        NodeDescriptor::from_str(v).map_err(|e| E::custom(e.to_string()))
    }
}
impl FromStr for NodeDescriptor {
    type Err = super::node_id::ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.contains(NodeIdTyped::DELIM) {
            NodeIdTyped::from_str(s).map(NodeDescriptor::from)
        } else {
            NodePathTyped::from_str(s)
                .map(NodeDescriptor::from)
                .map_err(Self::Err::from)
        }
    }
}

// TODO is this needed?   deleteme
// impl NodeDescriptor {
//     /// Converts to a [`NodePathTyped`]
//     pub fn into_path(self) -> NodePathTyped {
//         self.into()
//     }
//     /// Attempts to convert to [`NodeIdTyped`]
//     ///
//     /// # Errors
//     /// Returns the [`NodePathTyped`] if not an ID
//     pub fn try_into_id(self) -> Result<NodeIdTyped, NodePathTyped> {
//         self.try_into()
//     }
// }
impl From<NodeDescriptor> for NodePathTyped {
    fn from(desc: NodeDescriptor) -> Self {
        match desc {
            NodeDescriptor::Path(node_path) => node_path,
            NodeDescriptor::Id(node_id) => node_id.into(),
        }
    }
}
impl TryFrom<NodeDescriptor> for NodeIdTyped {
    type Error = NodePathTyped;

    fn try_from(value: NodeDescriptor) -> Result<Self, Self::Error> {
        match value {
            NodeDescriptor::Path(node_path) => Err(node_path),
            NodeDescriptor::Id(node_id) => Ok(node_id),
        }
    }
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use crate::id::{Keeper, NodePathTyped};

    use super::NodeDescriptor;
    use serde_json::Result;

    macro_rules! test {
        ($input_str:expr, [$($expect_path:expr),*] $(seq $expect_seq:expr)? ) => {
               {
                   let input_str = $input_str;
                   let parsed = serde_json::from_str(&format!("\"{input_str}\""))?;
                   let expect_path = NodePathTyped::from(vec![ $($expect_path),* ]);
                   test!(@parsed input_str, parsed, expect_path, $(seq $expect_seq)?);
               }
        };
        (@parsed $input_str:expr, $parsed:expr, $expect_path:expr, seq $expect_seq:expr) => {
            let input_str = $input_str;
            let expect_seq = $expect_seq;
            match $parsed {
                NodeDescriptor::Id(node_id) => {
                    assert_eq!(
                        node_id,
                        $expect_path
                            .with_sequence(&Keeper::assert_valid_sequence_from_user(expect_seq)),
                    );
                }
                other => panic!("unexpected result {other:?} for input {input_str:?}"),
            }
        };
        (@parsed $input_str:expr, $parsed:expr, $expect_path:expr $(,)?) => {
            let input_str = $input_str;
            match $parsed {
                NodeDescriptor::Path(node_path) => {
                    assert_eq!(node_path, $expect_path);
                }
                other => panic!("unexpected result {other:?} for input {input_str:?}"),
            }
        };
    }

    #[test]
    fn deserialize_simple() -> Result<()> {
        test!(".", []);
        test!(".#27", [] seq 27);
        Ok(())
    }
    #[test]
    fn deserialize_complex() -> Result<()> {
        test!(".0.2.3.7#5", [0, 2, 3, 7] seq 5);
        test!(".1.2.3", [1, 2, 3]);
        test!(".3.5.7.9.22.9002.39438#22", [3, 5, 7, 9, 22, 9002, 39438] seq 22);
        Ok(())
    }
}
