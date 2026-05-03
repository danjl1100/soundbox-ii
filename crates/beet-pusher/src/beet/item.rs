// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use super::BeetPath;

const SEPARATOR: &str = "=";

/// Beet library item (path and id) from a beet query
///
/// # Example
/// ```
/// use beet_pusher::BeetItem;
///
/// let item: BeetItem = "52=/path/to/item".parse().expect("valid item string");
/// assert_eq!(item.get_beet_id(), 52);
/// assert_eq!(item.get_path().as_str(), "/path/to/item");
/// ```
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct BeetItem {
    beet_id: u64,
    path: BeetPath,
}
impl BeetItem {
    /// Returns the beet ID
    #[must_use]
    pub fn get_beet_id(&self) -> u64 {
        self.beet_id
    }
    /// Returns the beet library path
    #[must_use]
    pub fn get_path(&self) -> &BeetPath {
        &self.path
    }
    /// Creates an item from an unchecked path and ID
    #[must_use]
    pub fn new_unchecked(beet_id: u64, path: String) -> Self {
        let path = BeetPath::new(path);
        Self { beet_id, path }
    }
    fn parse_id_path(s: &str) -> Result<Self, Error> {
        let make_err = |kind| Error { kind };

        let Some((beet_id, path)) = s.split_once(SEPARATOR) else {
            return Err(make_err(ErrorKind::MissingSeparator {
                separator: SEPARATOR,
            }));
        };
        let beet_id = beet_id
            .parse()
            .map_err(|source| ErrorKind::InvalidId {
                source,
                id_string: beet_id.to_string(),
            })
            .map_err(make_err)?;
        let path = BeetPath::new(path.to_string());
        Ok(Self { beet_id, path })
    }
}
impl std::fmt::Display for BeetItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { beet_id, path } = self;
        let path = path.as_str();
        write!(f, "{beet_id}{SEPARATOR}{path}")
    }
}

impl std::str::FromStr for BeetItem {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        BeetItem::parse_id_path(s)
    }
}
impl AsRef<BeetPath> for BeetItem {
    fn as_ref(&self) -> &BeetPath {
        self.get_path()
    }
}

/// Invalid [`BeetItem`] specification from beet
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct Error {
    kind: ErrorKind,
}
#[derive(Debug, thiserror::Error)]
enum ErrorKind {
    #[error("missing separator: {separator:?}")]
    MissingSeparator { separator: &'static str },
    #[error("invalid id number: {id_string:?}")]
    InvalidId {
        source: std::num::ParseIntError,
        id_string: String,
    },
}
