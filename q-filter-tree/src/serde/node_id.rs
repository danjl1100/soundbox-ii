// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use std::str::FromStr;

use serde::{
    de::{Error, Visitor},
    Deserialize, Serialize,
};

use crate::id::{Keeper, NodeIdTyped, NodePathRefTyped, NodePathTyped, Sequence, SequenceSource};

impl NodeIdTyped {
    const DELIM: &'static str = "#";
    const SERIALIZED_ADDENDUM: &'static str = "pound sign and uint (sequence element)";
}
impl std::fmt::Display for NodeIdTyped {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let path = NodePathRefTyped::from(self);
        let delim = Self::DELIM;
        let sequence = self.sequence();
        write!(f, "{path}{delim}{sequence}")
    }
}
impl Serialize for NodeIdTyped {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_str(&format_args!("{self}"))
    }
}

impl<'de> Deserialize<'de> for NodeIdTyped {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_string(NodeIdVisitor)
    }
}
struct NodeIdVisitor;
impl<'de> Visitor<'de> for NodeIdVisitor {
    type Value = NodeIdTyped;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        let path_description = NodePathTyped::SERIALIZED_DESCRIPTION;
        let id_addendum = NodeIdTyped::SERIALIZED_ADDENDUM;
        formatter.write_str(&format!("{path_description}, followed by {id_addendum}"))
    }
    fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
        NodeIdTyped::from_str(v).map_err(|e| E::custom(e.to_string()))
    }
}
impl FromStr for NodeIdTyped {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (path_str, seq_str) = {
            let mut parts = value.split(Self::DELIM);
            let path_str = parts.next().ok_or(ParseError::MissingIdDelim)?;
            let seq_str = parts.next().ok_or(ParseError::MissingIdDelim)?;
            (path_str, seq_str)
        };
        let path = NodePathTyped::from_str(path_str)?;
        let sequence = Sequence::from_str(seq_str)
            .map_err(|e| ParseError::InvalidIdInt(e, seq_str.to_string()))?;
        let sequence = Keeper::assert_valid_sequence_from_user(sequence);
        Ok(path.with_sequence(&sequence))
    }
}

shared::wrapper_enum! {
    pub enum ParseError {
        /// Error parsing the path
        PathError(super::NodePathParseError),
        { impl None for }
        InvalidIdInt(std::num::ParseIntError, String),
        /// Missing the ID delimiter
        MissingIdDelim,
    }
}
impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PathError(inner) => {
                write!(f, "invalid path specifier: {inner}")
            }
            Self::InvalidIdInt(_, fail_seq_str) => {
                write!(f, "invalid sequence number \"{fail_seq_str}\"")
            }
            Self::MissingIdDelim => {
                let delim = NodeIdTyped::DELIM;
                write!(f, "missing node-id delimiter \"{delim}\"")
            }
        }
    }
}
