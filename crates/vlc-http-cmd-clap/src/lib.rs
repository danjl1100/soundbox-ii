// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! [`clap`] compatible versions of [`vlc_http_cmd`] types

// re-export `clap`
pub use ::clap as clap_crate;

use vlc_http_cmd::url::Url;

pub mod command;
pub mod goal;
