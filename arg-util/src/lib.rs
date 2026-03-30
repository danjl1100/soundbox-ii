// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Helper utilities for parsing and validating arguments

pub use arg_split::ArgSplit;
mod arg_split;

pub use arg_join::{
    DisplayAsDebug, StrDebugSplit, StringDebugSplit, join_debug_by_spaces, join_display_by_spaces,
};
mod arg_join;

pub use multi_source::{Input, Source, Value};
pub mod multi_source;
