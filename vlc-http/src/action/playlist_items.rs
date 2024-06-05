// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use super::{
    playback_mode, query_playback::QueryPlayback, query_playlist::QueryPlaylist, Error, Poll,
    PollableConstructor,
};
use crate::{action::PlaybackMode, Command, Pollable};

#[derive(Debug)]
pub(crate) struct Set {
    target: Target,
    playback_mode: playback_mode::Set,
    query_playback: QueryPlayback,
    query_playlist: QueryPlaylist,
}
#[derive(Debug)]
pub(crate) struct Target {
    /// NOTE: The first element of `urls` is accepted as previously-played if it is the most recent history item.
    pub urls: Vec<url::Url>,
    // TODO
    // pub max_history_count: std::num::NonZeroU16,
}

impl Pollable for Set {
    type Output<'a> = ();

    fn next(&mut self, state: &crate::ClientState) -> Result<Poll<()>, Error> {
        match self.playback_mode.next(state)? {
            Poll::Done(()) => {}
            Poll::Need(endpoint) => return Ok(Poll::Need(endpoint)),
        }

        let playback = match self.query_playback.next(state)? {
            Poll::Done(playback) => playback,
            Poll::Need(endpoint) => return Ok(Poll::Need(endpoint)),
        };

        let playlist = match self.query_playlist.next(state)? {
            Poll::Done(playlist) => playlist,
            Poll::Need(endpoint) => return Ok(Poll::Need(endpoint)),
        };

        let next = if let Some(current_item_id) = playback
            .information
            .as_ref()
            .and_then(|info| info.playlist_item_id)
        {
            dbg!(current_item_id);
            // TODO
            todo!()
        } else {
            let playlist_urls: Vec<_> = playlist.iter().map(|item| &item.url).collect();
            find_next_to_insert(&self.target.urls, &playlist_urls)
        };

        if let Some(next) = next {
            return Ok(Poll::Need(
                Command::PlaylistAdd { url: next.clone() }.into(),
            ));
        }

        Ok(Poll::Done(()))
    }
}

/// Search for the *beginning* of `target` at the *end* of `existing`
///
/// Returns the next element in `target` that should be appended to `existing`, for the goal of
/// `existing` to end with all elements of `target` in-order
fn find_next_to_insert<'a, T>(target: &'a [T], existing: &[&T]) -> Option<&'a T>
where
    T: Eq + std::fmt::Debug,
{
    // trim existing (prefix longer than `target` does not matter)
    let existing = existing
        .len()
        .checked_sub(target.len())
        .map_or(existing, |excess_existing_len| {
            &existing[excess_existing_len..]
        });

    // search for perfect match
    if target.len() == existing.len()
        && target
            .iter()
            .zip(existing.iter())
            .all(|(target, &existing)| target == existing)
    {
        // perfect match, nothing to add
        return None;
    }

    // search for partial matches
    for (remaining, next) in target.iter().enumerate().skip(1).take(existing.len()).rev() {
        let existing = &existing[(existing.len() - remaining)..];

        if target
            .iter()
            .zip(existing.iter())
            .all(|(target, &existing)| target == existing)
        {
            // partial match, add the next
            return Some(next);
        }
    }
    // no partial matches found, begin by adding the first (if any)
    target.first()
}

impl PollableConstructor for Set {
    type Args = Target;
    fn new(target: Self::Args, state: &crate::ClientState) -> Self {
        const LINEAR_PLAYBACK: PlaybackMode = PlaybackMode::new()
            .set_repeat(crate::action::RepeatMode::Off)
            .set_random(false);
        Self {
            target,
            playback_mode: playback_mode::Set::new(LINEAR_PLAYBACK, state),
            query_playback: QueryPlayback::new((), state),
            query_playlist: QueryPlaylist::new((), state),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_next() {
        let needle = &[1, 2, 3, 4];
        println!("0");
        assert_eq!(find_next_to_insert(needle, &[]), Some(&1));
        println!("1");
        assert_eq!(find_next_to_insert(needle, &[&1]), Some(&2));
        println!("2");
        assert_eq!(find_next_to_insert(needle, &[&1, &2]), Some(&3));
        println!("3");
        assert_eq!(find_next_to_insert(needle, &[&1, &2, &3]), Some(&4));
        println!("4");
        assert_eq!(find_next_to_insert(needle, &[&1, &2, &3, &4]), None);
        println!("5");
        assert_eq!(find_next_to_insert(needle, &[&10, &1, &2, &3, &4]), None);
        println!("6");
        assert_eq!(
            find_next_to_insert(needle, &[&10, &10, &1, &2, &3, &4]),
            None
        );

        println!("7");
        assert_eq!(find_next_to_insert(needle, &[&1, &2, &3, &4, &1]), Some(&2));

        println!("8");
        assert_eq!(find_next_to_insert(needle, &[&10, &10, &1, &2]), Some(&3));
    }
}
