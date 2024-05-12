// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//
//! High-level actions for VLC (correspond to a single API call)

use crate::{client_state::Sequence, response, ClientState, Endpoint};

mod query_playback;
mod query_playlist;

/// High-level actions to control VLC (dynamic API calls depending on the current state)
///
/// NOTE: The enum variants are for non-query actions (think `Result<(), Error>`)
///
/// See the inherent functions for queries with specific return results
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    // TODO
    // /// Set the current playing and up-next playlist URLs, clearing the history to the specified max count
    // ///
    // /// NOTE: The first element of `urls` is accepted as previously-played if it is the most recent history item.
    // /// NOTE: Forces the playback mode to `{ repeat: RepeatMode::Off, random: false }`
    // PlaylistSet {
    //     /// Path to the file(s) to queue next, starting with the current/past item
    //     urls: Vec<url::Url>,
    //     /// Maximum number of history (past-played) items to retain
    //     ///
    //     /// NOTE: Enforced as non-zero, since at least 1 "history" item is needed to:
    //     ///  * detect the "past" case of `current_or_past_url`, and
    //     ///  * add current the playlist (to retain during the 1 tick where current is added, but not yet playing)
    //     max_history_count: NonZeroUsize,
    // },
    // /// Set the item selection mode
    // PlaybackMode {
    //     #[allow(missing_docs)]
    //     repeat: RepeatMode,
    //     /// Randomizes the VLC playback order when `true`
    //     random: bool,
    // },
}
// /// Rule for selecting the next playback item in the VLC queue
// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
// pub enum RepeatMode {
//     /// Stop the VLC queue after playing all items
//     Off,
//     /// Repeat the VLC queue after playing all items
//     All,
//     /// Repeat only the current item
//     One,
// }
// impl RepeatMode {
//     pub(crate) fn is_loop_all(self) -> bool {
//         self == Self::All
//     }
//     pub(crate) fn is_repeat_one(self) -> bool {
//         self == Self::One
//     }
// }

#[allow(unused)] // TODO
/// [`Pollable`] container for various (non-query) [`Action`]s
#[allow(clippy::module_name_repetitions)]
pub struct ActionPollable {
    inner: ActionPollableInner,
}
#[allow(unused)] // TODO
enum ActionPollableInner {
    // TODO
}

impl Action {
    /// Returns an endpoint source for querying the playlist info
    #[must_use]
    #[allow(clippy::needless_lifetimes)] // reference <https://github.com/rust-lang/rust-clippy/issues/11291>
    pub fn query_playlist<'a>(
        state: &'a ClientState,
    ) -> impl Pollable<Output<'a> = &'a [response::playlist::Item]> + 'static {
        query_playlist::QueryPlaylist::new((), state)
    }
    /// Returns an endpoint source for querying the playlist info
    #[must_use]
    pub fn query_playback<'a>(
        state: &ClientState,
    ) -> impl Pollable<Output<'a> = &'a response::PlaybackStatus> + 'static {
        query_playback::QueryPlayback::new((), state)
    }
}

/// Sequence of endpoints required to calculated the output
pub trait Pollable: std::fmt::Debug {
    /// Final output when no more endpoints are needed
    type Output<'a>: std::fmt::Debug;

    /// Returns an [`Endpoint`] to make progress on the action on the [`ClientState`]
    ///
    /// # Errors
    /// Returns an error describing why no further actions are possible.
    ///
    /// The error may contain a query result (for queries), or an error (for non-queries)
    fn next_endpoint<'a>(&mut self, state: &'a ClientState) -> Result<Endpoint, Self::Output<'a>>;
}
trait PollableConstructor: Pollable
where
    // NOTE: `Serialize` is for tests... hopefully not too invasive?
    for<'a> Self::Output<'a>: serde::Serialize,
{
    type Args;
    fn new(args: Self::Args, state: &ClientState) -> Self;
}
