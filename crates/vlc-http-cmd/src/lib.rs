// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Command and goal input types for controlling VLC, separate from the core `vlc-http` logic.
//!
//! - [`Command`] — low-level, maps 1:1 to a single VLC API call
//! - [`Goal`] — high-level desired state, may require multiple API calls
//! - [`url`] — re-exported for constructing URLs in commands and goals
//!
//! NOTE: See `vlc_http::client_state::PlanBuilder` for options to query information

pub use ::url;

pub use self::command::Command;
pub use self::goal::Goal;

pub mod command;
pub mod goal;
pub mod url_fmt;
