// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Helper utilities for parsing and validating arguments

pub use self::arg_split::ArgSplit;
mod arg_split;

pub use self::arg_join::{
    DisplayAsDebug, StrDebugSplit, StringDebugSplit, join_debug_by_spaces, join_display_by_spaces,
};
mod arg_join;

pub use self::multi_source::{Input, Source, Value};
pub mod multi_source;

pub use config_file::{ConfigFileOpen, ConfigFileWrite};
pub mod config_file;
