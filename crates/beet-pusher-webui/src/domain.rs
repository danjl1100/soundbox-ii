// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Domain-specific logic and types
// TODO: "with no specific knowledge about the backend" (see issues/13-webui-architecture-review.md)

pub mod services;

pub mod models {
    //! Domain types

    pub use beet_pusher::pipe_exec::NodePath;
}
