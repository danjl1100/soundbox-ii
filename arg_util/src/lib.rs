// Copyright (C) 2021-2023  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Helper utilities for parsing and validating arguments

// teach me
#![deny(clippy::pedantic)]
#![allow(clippy::bool_to_int_with_if)] // except this confusing pattern
// no unsafe
#![forbid(unsafe_code)]
// no unwrap
#![deny(clippy::unwrap_used)]
// no panic
#![deny(clippy::panic)]
// docs!
#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

pub use arg_split::ArgSplit;
mod arg_split;

pub use multi_source::{Input, Source, Value};
pub mod multi_source;
