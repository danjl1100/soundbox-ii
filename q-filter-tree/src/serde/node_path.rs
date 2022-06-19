// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use crate::id::{NodePathElem, NodePathRefTyped, NodePathTyped};

use serde::de::{Deserialize, Deserializer, Error, Visitor};
use serde::ser::{Serialize, Serializer};
use std::str::FromStr;

impl NodePathTyped {
    const DELIM: &'static str = ".";
    const START_DELIM: &'static str = Self::DELIM;
    pub(super) const SERIALIZED_DESCRIPTION: &'static str =
        "string of dot-separated uints (path elements)";
}
// NOTE: Serialize defined on `NodePathRefTyped` as lowest-common-denominator
// for both `NodePathTyped` and `NodeIdTyped` to benefit.
impl<'a> std::fmt::Display for NodePathRefTyped<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let start_delim = NodePathTyped::START_DELIM;
        let delim = NodePathTyped::DELIM;
        write!(f, "{start_delim}")?;
        let mut first = true;
        for elem in &**self {
            if first {
                write!(f, "{elem}")?;
            } else {
                write!(f, "{delim}{elem}")?;
            }
            first = false;
        }
        Ok(())
    }
}
impl<'a> Serialize for NodePathRefTyped<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(&format_args!("{self}"))
    }
}
// Inherit Display/Serialize from `NodePathRefTyped`
impl std::fmt::Display for NodePathTyped {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let r = NodePathRefTyped::from(self);
        write!(f, "{r}")
    }
}
impl Serialize for NodePathTyped {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        NodePathRefTyped::from(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for NodePathTyped {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_string(NodePathVisitor)
    }
}
struct NodePathVisitor;
impl<'de> Visitor<'de> for NodePathVisitor {
    type Value = NodePathTyped;
    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str(NodePathTyped::SERIALIZED_DESCRIPTION)
    }
    fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
        NodePathTyped::from_str(v).map_err(|e| E::custom(e.to_string()))
    }
}
impl FromStr for NodePathTyped {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let content = {
            if !value.starts_with(Self::START_DELIM) {
                return Err(ParseError::MissingStartDelimiter);
            }
            &value[1..]
        };
        let node_path_elems = match content {
            // empty string --> empty list
            "" => Ok(vec![]),
            // split on separator
            elems_str => elems_str
                .split(NodePathTyped::DELIM)
                // parse int
                .map(|elem| {
                    NodePathElem::from_str(elem)
                        .map_err(|err| ParseError::InvalidPathInt(err, elem.to_owned()))
                })
                .collect::<Result<Vec<NodePathElem>, _>>(),
        };
        node_path_elems.map(NodePathTyped::from)
    }
}
pub enum ParseError {
    MissingStartDelimiter,
    InvalidPathInt(std::num::ParseIntError, String),
}
impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::MissingStartDelimiter => {
                let start_delim = NodePathTyped::START_DELIM;
                write!(f, "missing start delimiter \"{start_delim}\"")
            }
            Self::InvalidPathInt(_, fail_elem_str) => {
                write!(f, "invalid path element \"{fail_elem_str}\"")
            }
        }
    }
}
