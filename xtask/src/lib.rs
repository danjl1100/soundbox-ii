// Copyright (C) 2021-2025  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Logic for the `xtask` functionality

pub mod copyright;

/// If present, attempt to fix the checks by writing to files (otherwise, run read-only checks)
#[derive(Clone, Copy, Debug)]
pub struct Fix;

/// If present, write output files (otherwise, run read-only checks)
#[derive(Clone, Copy, Debug)]
pub struct WriteOutput;
