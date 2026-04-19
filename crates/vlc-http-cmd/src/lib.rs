// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Separates the command/goal input layer from the core `vlc-http` logic
//!
//! Provides a reexport of [`url`] (intrinsic to commands)

pub use ::url;

pub use self::command::Command;
pub use self::goal::Change;

pub mod command;
pub mod fmt;
pub mod goal;
