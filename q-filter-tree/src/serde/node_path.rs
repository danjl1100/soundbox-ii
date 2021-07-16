use crate::id::{NodePath, NodePathElem};

use serde::de::{Deserialize, Deserializer, Error, Visitor};
use serde::ser::{Serialize, Serializer};
use std::str::FromStr;

impl NodePath {
    const DELIM: &'static str = ",";
}
impl std::fmt::Display for NodePath {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let mut first = true;
        for elem in self.elems() {
            if first {
                write!(f, "{}", elem)?;
            } else {
                write!(f, "{}{}", Self::DELIM, elem)?;
            }
            first = false;
        }
        Ok(())
    }
}

impl Serialize for NodePath {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(&format_args!("{}", &self))
    }
}

impl<'de> Deserialize<'de> for NodePath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_string(NodePathVisitor)
    }
}
struct NodePathVisitor;
impl<'de> Visitor<'de> for NodePathVisitor {
    type Value = NodePath;
    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("string of comma separated uints (path elements)")
    }
    fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
        type ParseIntErrorAndStr<'a> = (std::num::ParseIntError, &'a str);
        match v {
            // empty string --> empty list
            "" => Ok(vec![]),
            // split on separator
            elems_str => elems_str
                .split(NodePath::DELIM)
                // parse int
                .map(|elem| NodePathElem::from_str(elem).map_err(|err| (err, elem)))
                .collect::<Result<Vec<NodePathElem>, ParseIntErrorAndStr<'_>>>(),
        }
        .map(NodePath::from)
        .map_err(|(_, fail_elem_str)| {
            E::custom(format!("invalid path element \"{}\"", fail_elem_str))
        })
    }
}
