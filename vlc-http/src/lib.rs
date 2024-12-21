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
//! A [sans-io](https://sans-io.readthedocs.io/) library for encoding commands and parsing
//! responses for the ubiquitous [VLC](https://www.videolan.org/vlc/) media player's
//! [HTTP interface](https://wiki.videolan.org/VSG:Interface:HTTP)
//!
//!
//! # Philosophy
//!
//! Where possible, this library chooses to use *data* for commands and goals instead of function
//! calls.  This allows for easier RPC, and even cli debugging using the [`crate::clap`] helpers.
//!
//! Who needs "mocks" for testing when the business-logic control flow is plan old data?
//!
//!
//! # Overview
//!
//! The illustration below shows the general flow for an application using this library:
//!
//! ```text
//!
//!    (*) START: application wants to control or query VLC
//!     v
//!   ----------------
//!  | 0. ClientState |
//!   ----------------
//!     v
//!     v < < < < < < < < < < < < < < < < < < < < < < < < < < < < < < < < < < < < < < <
//!     v                                                                             ^
//!   ---------      ------------      -------------                                  ^
//!  | 1. Plan | -> | 2. Command | -> | 3. Endpoint |                                 ^
//!   ---------  |   ------------      -------------                                  ^
//!             [OR]                         v                                        ^
//!              |                          (*) application sends HTTP request        ^
//!              -> (*) Query result         v                                        ^
//!                                       [ VLC ]                                     ^
//!                                          v                                        ^
//!                                         (*) application receives HTTP response    ^
//!                                          v                                        ^
//!                                    -------------      --------------------        ^
//!                                   | 4. Response | -> | update ClientState | -> repeat
//!                                    -------------      --------------------
//! ```
//!
//! The components are described in detail below:
//!
//!
//! ## 0. [`ClientState`]
//! A [`ClientState`] instance tracks the current state of one VLC instance, for the purpose of speeding up
//! commands and queries. The cache is used or invalidated based on the creation of the plans.
//!
//! ## 1. [`Plan`]
//! A [`Plan`] completes a high-level action through a series of steps, depending on the updates to
//! [`ClientState`] from each step.
//! This is needed for more complex state-dependent commands.
//!
//! ## 2. [`Command`]
//! [`Command`]s provide low-level control of the player, suitable for basic playback
//! control ([`PlaybackResume`], [`PlaybackPause`], [`SeekNext`], [`SeekPrevious`]).
//!
//! ## 3. [`Endpoint`]
//! #TODO
//!
//! ## 4. [`Response`]
//! #TODO
//!
//! #TODO
//! ... Then, use the response to update the client state. The update returns a sequence marker
//! than can be used to extend caching for future commands or queries. That is, later the
//! application wants to query the **cached** playback status, then it provides the earlier
//! sequence marker and the HTTP request should not needed.
//!
//! The application has to provide proof of some prior update in order to use the cached results.
//! Obviously, if new data arrived since then, the newer data will be used.
//!
//!
//! [`PlaybackResume`]: `Command::PlaybackResume`
//! [`PlaybackPause`]: `Command::PlaybackPause`
//! [`SeekNext`]: `Command::SeekNext`
//! [`SeekPrevious`]: `Command::SeekPrevious`

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

pub use goal::{Change, Plan};
pub mod goal;

pub use request::{Auth, Endpoint};
pub mod request;

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
