// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! [`BeetPusher`] implements `beet` <--> `VLC` logic using a [`bucket_spigot::Network`]:
//! 1. queryies [`BeetItem`]s from beet to fill a [`bucket_spigot::Network`]
//! 2. pushes items from the spigot to [`vlc_http`]

pub use self::beet::{BeetCommand, BeetItem, BeetPath, BeetRunner, fill_buckets};
pub use self::determined::{Determined, UrlSource};
pub use self::path_url::BaseUrl;
pub use self::pusher::{
    BeetPusher, FillDeterminedError, HintNeedPlaylistUpdate, NowPlayingObserver,
};

pub mod beet;

mod determined;
mod path_url;
mod pusher;

pub mod pipe_exec;

pub mod command_loop;

/// Signal to shutdown the application
struct Shutdown;
