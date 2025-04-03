// Copyright (C) 2021-2025  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
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
#[derive(Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(transparent)]
#[allow(clippy::module_name_repetitions)]
pub struct PathRef<'a>(#[serde(serialize_with = "path_elems_serialize")] &'a [usize]);

impl Path {
    /// Constructs an empty [`Path`]
    pub const fn empty() -> Self {
        Self(vec![])
    }
    /// Borrows the path
    #[must_use]
    pub fn as_ref(&self) -> PathRef<'_> {
        let Self(elems) = self;
        PathRef(elems)
    }
    /// Returns an iterator for path elements
    pub fn iter(&self) -> Iter<'_> {
        self.as_ref().into_iter()
    }
    /// Returns the length of the path
    #[must_use]
    pub fn len(&self) -> usize {
        self.as_ref().len()
    }
    /// Returns whether the path is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.as_ref().is_empty()
    }
    /// Appends a path element
    pub fn push(&mut self, elem: usize) {
        self.0.push(elem);
    }
    /// Removes the last path element (if any)
    pub fn pop(&mut self) -> Option<usize> {
        self.0.pop()
    }
    /// Modify the path as-if the specified path was removed from the network
    ///
    /// e.g. If this path is a "greater" sibling of the removed path, then decrement this path
    pub(crate) fn modify_for_removed(&mut self, removed: PathRef<'_>) -> Result<(), RemovedSelf> {
        use std::cmp::Ordering;

        let mut this = self.0.iter_mut().peekable();
        let mut other = removed.iter().peekable();
        #[expect(clippy::needless_continue)]
        while let Some((this_elem, other_elem)) = this.next().zip(other.next()) {
            let other_ended = other.peek().is_none();
            match other_elem.cmp(this_elem) {
                Ordering::Equal if other_ended => {
                    return Err(RemovedSelf);
                }
                Ordering::Less if other_ended => {
                    // apply modification if "other < this" at the end
                    // (no panic, `integer < this` ensures nonzero)
                    *this_elem -= 1;
                    break;
                }
                // related path, continue comparison
                Ordering::Equal => continue,
                // non-related path, end comparison
                Ordering::Greater | Ordering::Less => break,
            }
        }
        Ok(())
    }
}
impl From<Vec<usize>> for Path {
    fn from(value: Vec<usize>) -> Self {
        Self(value)
    }
}
impl FromIterator<usize> for Path {
    fn from_iter<T: IntoIterator<Item = usize>>(iter: T) -> Self {
        let elems: Vec<usize> = iter.into_iter().collect();
        elems.into()
    }
}

impl PathRef<'static> {
    /// Constructs an immutable reference to the root path
    ///
    /// NOTE: Must [convert to owned](`PathRef::to_owned`) to append elements
    #[allow(clippy::must_use_candidate)]
    pub const fn empty() -> Self {
        Self(&[])
    }
}
impl<'a> PathRef<'a> {
    /// Returns an iterator for path elements
    pub fn iter(self) -> Iter<'a> {
        self.into_iter()
    }
    /// Returns the length of the path
    #[must_use]
    pub fn len(self) -> usize {
        self.0.len()
    }
    /// Returns whether the path is empty
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.0.is_empty()
    }
    /// Returns the last element and the rest (if any)
    #[must_use]
    pub fn split_last(self) -> Option<(usize, Self)> {
        let (&last, rest) = self.0.split_last()?;
        Some((last, Self(rest)))
    }
    /// Clones the inner [`Path`]
    pub fn to_owned(self) -> Path {
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
impl std::fmt::Debug for PathRef<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PathRef({self})")
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

/// Error modifying a path: the removed path matches self
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RemovedSelf;

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
    Path::from_str(input)
        .map(|Path(elems)| elems)
        .map_err(serde::de::Error::custom)
}
