// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! API handlers and extractors

pub mod extractors;
pub mod handlers;

pub use self::json_out::{JsonOut, ResponseOut};
mod json_out;
