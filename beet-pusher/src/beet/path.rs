/// Beet library path
#[derive(Clone, Debug, serde::Serialize)]
pub struct BeetPath(
    // NOTE: not `PathBuf` because we already entered UTF-8 land by parsing Beet output
    //       The string may need further modifications to represent a real path
    String,
);
impl BeetPath {
    #[must_use]
    pub(super) fn new(path: String) -> Self {
        Self(path)
    }
    /// Returns the string representation of the beet library path
    #[must_use]
    pub fn as_str(&self) -> &str {
        let Self(path) = self;
        path
    }
}
