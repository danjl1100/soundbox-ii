//! Shared payload types passed from backend to frontend.

// TODO: only while building
#![allow(dead_code)]
// teach me
#![deny(clippy::pedantic)]
// no unsafe
#![forbid(unsafe_code)]
// no unwrap
#![deny(clippy::unwrap_used)]
// no panic
#![deny(clippy::panic)]
// docs!
#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

use serde::{Deserialize, Serialize};

/// Testing "awesome number" type
#[allow(missing_docs)]
#[derive(Debug, Deserialize, Serialize)]
pub struct Number {
    pub value: u32,
    pub title: String,
    pub is_even: bool,
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
