// Copyright (C) 2021-2025  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Proof of concept UI for visualizing the nodes and items flowing through a [`bucket_spigot`]
//! and modify the network.

/// HTTP Code 101
pub const HTTP_CODE_101_SWITCHING_PROTOCOLS: u32 = 101;
/// HTTP Code 301
pub const HTTP_CODE_301_MOVED: u32 = 301;
/// HTTP Code 404
pub const HTTP_CODE_404_NOT_FOUND: u32 = 404;

pub mod static_file;

pub mod websocket;
