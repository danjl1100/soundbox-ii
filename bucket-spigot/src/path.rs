// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Location-dependent identifier for nodes

use serde::Deserialize;
use std::str::FromStr;

const DELIMITER: &str = ".";

/// Path to a node (joint or bucket) in the [`Network`](`crate::Network`)
#[derive(Clone, PartialEq, Hash, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
#[must_use]
pub struct Path(
    #[serde(
        serialize_with = "path_elems_serialize",
        deserialize_with = "path_elems_deserialize"
    )]
    Vec<usize>,
);

/// Borrow of a [`Path`]
#[must_use]
#[derive(Clone, Copy, PartialEq, Eq)]
#[allow(clippy::module_name_repetitions)]
pub struct PathRef<'a>(&'a [usize]);

impl Path {
    /// Borrows the path
    pub fn as_ref(&self) -> PathRef<'_> {
        let Self(elems) = self;
        PathRef(elems)
    }
    /// Returns an iterator for path elements
    pub fn iter(&self) -> Iter<'_> {
        self.as_ref().into_iter()
    }
    /// Appends a path element
    pub fn push(&mut self, elem: usize) {
        self.0.push(elem);
    }
    /// Removes the last path element (if any)
    pub fn pop(&mut self) -> Option<usize> {
        self.0.pop()
    }
}
impl From<Vec<usize>> for Path {
    fn from(value: Vec<usize>) -> Self {
        Self(value)
    }
}

impl PathRef<'_> {
    /// Returns the last element and the rest (if any)
    #[must_use]
    pub fn split_last(self) -> Option<(usize, Self)> {
        let (&last, rest) = self.0.split_last()?;
        Some((last, Self(rest)))
    }
    /// Clones the inner [`Path`]
    pub fn clone_inner(self) -> Path {
        Path(self.0.to_vec())
    }
}

/// Iterator for a [`Path`]
#[must_use]
pub struct Iter<'a>(std::slice::Iter<'a, usize>);
impl Iterator for Iter<'_> {
    type Item = usize;
    fn next(&mut self) -> Option<Self::Item> {
        let Self(iter) = self;
        iter.next().copied()
    }
}
impl<'a> IntoIterator for &'a Path {
    type Item = usize;
    type IntoIter = Iter<'a>;
    fn into_iter(self) -> Self::IntoIter {
        self.as_ref().into_iter()
    }
}
impl<'a> IntoIterator for PathRef<'a> {
    type Item = usize;
    type IntoIter = Iter<'a>;
    fn into_iter(self) -> Self::IntoIter {
        let Self(elems) = self;
        Iter(elems.iter())
    }
}

impl FromStr for Path {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            // must start with delimiter (nonempty)
            return Err(ErrorInner::MissingStartDelim.into());
        }
        if s == DELIMITER {
            return Ok(Self(vec![]));
        }
        let mut parts = s.split(DELIMITER);
        let Some("") = parts.next() else {
            // must start with delimiter (no leading text)
            return Err(ErrorInner::MissingStartDelim.into());
        };
        let elems = parts
            .map(|part| {
                part.parse().map_err(|_| ErrorInner::InvalidNumber {
                    input: part.to_owned(),
                })
            })
            .collect::<Result<Vec<_>, _>>();
        Ok(Self(elems?))
    }
}

impl std::fmt::Display for Path {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <PathRef as std::fmt::Display>::fmt(&self.as_ref(), f)
    }
}
impl std::fmt::Display for PathRef<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self(elems) = *self;
        if elems.is_empty() {
            write!(f, "{DELIMITER}")
        } else {
            for elem in elems {
                write!(f, "{DELIMITER}{elem}")?;
            }
            Ok(())
        }
    }
}
impl std::fmt::Debug for Path {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Path({self})")
    }
}

/// Error parsing a [`Path`]
#[derive(serde::Serialize)]
#[serde(transparent)]
pub struct Error(ErrorInner);

impl std::error::Error for Error {}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self(inner) = self;
        match inner {
            ErrorInner::MissingStartDelim => {
                write!(f, "missing start delimiter ({DELIMITER:?})")
            }
            ErrorInner::InvalidNumber { input } => write!(f, "invalid number: {input:?}"),
        }
    }
}
impl std::fmt::Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Error({self})")
    }
}

#[derive(serde::Serialize)]
enum ErrorInner {
    MissingStartDelim,
    InvalidNumber { input: String },
}
impl From<ErrorInner> for Error {
    fn from(value: ErrorInner) -> Self {
        Self(value)
    }
}

/// serialize slice of usize as if it were inside a [`Path`]
fn path_elems_serialize<S>(value: &[usize], serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.collect_str(&PathRef(value))
}

/// deserialize a Vec of usize as if it were inside a [`Path`]
fn path_elems_deserialize<'de, D>(deserializer: D) -> Result<Vec<usize>, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
    let input = <&str>::deserialize(deserializer)?;
    dbg!(input);
    Path::from_str(input)
        .map(|Path(elems)| elems)
        .map_err(serde::de::Error::custom)
}

#[cfg(test)]
mod tests {
    use super::{Error, Path};
    use std::str::FromStr as _;

    /// Exposes the structural contents of [`Path`], ignoring custom serialization
    #[derive(serde::Serialize, Debug)]
    #[serde(transparent)]
    struct PathStructural(Vec<usize>);

    fn path(input: &str) -> Result<Path, Error> {
        let result = Path::from_str(input);
        if let Ok(path_elems) = &result {
            // verify Display <==> FromStr
            assert_eq!(
                path_elems.to_string(),
                input,
                "from_str.to_string does not match input"
            );
        }
        result
    }
    fn path_elems(input: &str) -> Result<PathStructural, Error> {
        path(input).map(|Path(elems)| PathStructural(elems))
    }

    fn json_de_elems(input: &str) -> PathStructural {
        let Path(elems) = serde_json::from_str(input).expect("test JSON input should be valid");
        // verify Serialize <==> Deserialize
        // (note: restricts flexibility of test JSON inputs)
        assert_eq!(
            serde_json::to_string(&Path(elems.clone())).expect("should serialize OK"),
            input
        );
        PathStructural(elems)
    }

    #[test]
    fn inner_structure_from_str() {
        insta::assert_ron_snapshot!(path_elems(""), @"Err(MissingStartDelim)");
        insta::assert_ron_snapshot!(path_elems("invalid"), @"Err(MissingStartDelim)");

        insta::assert_ron_snapshot!(path_elems("invalid."), @"Err(MissingStartDelim)");
        insta::assert_ron_snapshot!(path_elems(".invalid"), @r###"
        Err(InvalidNumber(
          input: "invalid",
        ))
        "###);

        insta::assert_ron_snapshot!(path_elems("."), @"Ok([])");
        insta::assert_ron_snapshot!(path_elems(".1"), @r###"
        Ok([
          1,
        ])
        "###);
        insta::assert_ron_snapshot!(path_elems(".1.2.3.4.5"), @r###"
        Ok([
          1,
          2,
          3,
          4,
          5,
        ])
        "###);
        insta::assert_ron_snapshot!(dbg!(path_elems(".234.32.9")), @r###"
        Ok([
          234,
          32,
          9,
        ])
        "###);
    }

    #[test]
    fn public_ser() {
        insta::assert_ron_snapshot!(Path(vec![]), @r###"".""###);
        insta::assert_ron_snapshot!(Path(vec![1, 2, 3]), @r###"".1.2.3""###);

        insta::assert_ron_snapshot!(path(".1.2.3"), @r###"Ok(".1.2.3")"###);
    }
    #[test]
    fn public_de() {
        insta::assert_ron_snapshot!(json_de_elems("\".\""), @"[]");
        insta::assert_ron_snapshot!(json_de_elems("\".1\""), @r###"
        [
          1,
        ]
        "###);
        insta::assert_ron_snapshot!(json_de_elems("\".1.2.3.4.5\""), @r###"
        [
          1,
          2,
          3,
          4,
          5,
        ]
        "###);
        insta::assert_ron_snapshot!(json_de_elems("\".59.2.393904\""), @r###"
        [
          59,
          2,
          393904,
        ]
        "###);
    }
}
