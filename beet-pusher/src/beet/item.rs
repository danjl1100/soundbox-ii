use super::BeetPath;

const SEPARATOR: &str = "=";

/// Beet library item (path and id) from a beet query
#[derive(Clone, Debug, serde::Serialize)]
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
            return Err(make_err(ErrorKind::MissingSeparator));
        };
        let beet_id = beet_id
            .parse()
            .map_err(ErrorKind::InvalidId)
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
#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
}
#[derive(Debug)]
enum ErrorKind {
    MissingSeparator,
    InvalidId(std::num::ParseIntError),
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            ErrorKind::MissingSeparator => None,
            ErrorKind::InvalidId(error) => Some(error),
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { kind } = self;
        let description = match kind {
            ErrorKind::MissingSeparator => "missing separator",
            ErrorKind::InvalidId(_) => "invalid id number",
        };
        write!(f, "{description}")
    }
}
