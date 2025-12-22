//! Implements `beet` <--> `VLC` logic using a [`bucket_spigot::Network`]:
//! 1. queryies [`BeetItem`]s from beet to fill a [`bucket_spigot::Network`]
//! 2. pushes items from the spigot to [`vlc_http`]

pub use self::beet::{BeetItem, BeetPath, fill_buckets};
pub use self::determined::{Determined, UrlSource};
pub use self::path_url::BaseUrl;
pub use self::pusher::{BeetPusher, NowPlayingObserver};

/// Queries items from `beet`
mod beet;
mod determined;
mod path_url;
mod pusher;
