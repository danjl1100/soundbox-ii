// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

#![allow(clippy::panic)] // kind of the point of tests

pub use harness::run_input;
mod harness;

pub use model::Model;
pub mod model;
