// soundbox-ii/vlc-http VLC communication library *don't keep your sounds boxed up*
// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.
//
//! Encodes commands and parses events for the HTTP interface of VLC
//! ([sans-io](https://sans-io.readthedocs.io/))

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

mod fmt;

// --------------------------------------------------
// Requests sent to VLC
// --------------------------------------------------

pub use command::{Command, VolumePercent, VolumePercentDelta};
pub mod command;

pub use request::{Auth, Endpoint};
pub mod request;

pub use action::{Action, Pollable};
pub mod action;

// --------------------------------------------------
// Responses received from VLC
// --------------------------------------------------

pub use response::Response;
pub mod response;

pub use client_state::ClientState;
pub mod client_state;

// --------------------------------------------------
// Utilities
// --------------------------------------------------

#[cfg(feature = "clap")]
pub mod clap;

/// Helpers for specific HTTP client implementations
pub mod http_runner {
    #[cfg(feature = "ureq")]
    pub mod ureq;
}

pub mod sync;
