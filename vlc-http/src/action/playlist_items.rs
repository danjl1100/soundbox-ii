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
    pub max_history_count: std::num::NonZeroU16,
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

        let insert_match = if let Some(current_item_id) = playback
            .information
            .as_ref()
            .and_then(|info| info.playlist_item_id)
        {
            dbg!(current_item_id);
            // TODO
            todo!()
        } else {
            let playlist_urls: Vec<_> = playlist.iter().map(|item| &item.url).collect();
            find_insert_match(&self.target.urls, &playlist_urls)
        };

        // delete first entry to match `max_history_count`
        let match_start = insert_match.match_start.unwrap_or(playlist.len());
        let max_history_count = usize::from(self.target.max_history_count.get());
        if match_start > max_history_count && !playlist.is_empty() {
            return Ok(Poll::Need(
                Command::PlaylistDelete {
                    item_id: playlist[0].id.clone(),
                }
                .into(),
            ));
        }

        if let Some(next) = insert_match.next_to_insert {
            return Ok(Poll::Need(
                Command::PlaylistAdd { url: next.clone() }.into(),
            ));
        }

        Ok(Poll::Done(()))
    }
}

/// Search for the *beginning* of `target` at the *end* of `existing`
///
/// Returns the match index and the next element in `target` to append to `existing`,
/// for the goal of `existing` to end with all elements of `target` in-order
fn find_insert_match<'a, T>(target: &'a [T], existing: &[&T]) -> InsertMatch<'a, T>
where
    T: Eq + std::fmt::Debug,
{
    // trim existing (prefix longer than `target` does not matter)
    let start_offset = existing.len().saturating_sub(target.len());
    let existing = &existing[start_offset..];

    // search for perfect match
    if target.len() == existing.len()
        && target
            .iter()
            .zip(existing.iter())
            .all(|(target, &existing)| target == existing)
    {
        // perfect match, nothing to add
        return InsertMatch {
            match_start: Some(start_offset),
            next_to_insert: None,
        };
    }

    // search for partial matches
    for (remaining, next) in target.iter().enumerate().skip(1).take(existing.len()).rev() {
        let match_start = existing.len() - remaining;
        let existing = &existing[match_start..];

        if target
            .iter()
            .zip(existing.iter())
            .all(|(target, &existing)| target == existing)
        {
            // partial match, add the next
            return InsertMatch {
                match_start: Some(start_offset + match_start),
                next_to_insert: Some(next),
            };
        }
    }
    // no partial matches found, begin by adding the first (if any)
    InsertMatch {
        match_start: None,
        next_to_insert: target.first(),
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct InsertMatch<'a, T> {
    match_start: Option<usize>,
    next_to_insert: Option<&'a T>,
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

    fn insert_end<T>(next: &T) -> InsertMatch<'_, T> {
        InsertMatch {
            match_start: None,
            next_to_insert: Some(next),
        }
    }
    fn insert_from<T>(match_start: usize, next: &T) -> InsertMatch<'_, T> {
        InsertMatch {
            match_start: Some(match_start),
            next_to_insert: Some(next),
        }
    }
    fn matched<T>(match_start: usize) -> InsertMatch<'static, T> {
        InsertMatch {
            match_start: Some(match_start),
            next_to_insert: None,
        }
    }

    // NOTE tests are easier to read with this alias
    fn uut<'a, T>(target: &'a [T], existing: &[&T]) -> InsertMatch<'a, T>
    where
        T: std::fmt::Debug + Eq,
    {
        println!("target={target:?}, existing={existing:?}");
        find_insert_match(target, existing)
    }

    #[test]
    fn find_next() {
        let needle = &[1, 2, 3, 4];
        assert_eq!(uut(needle, &[]), insert_end(&1));
        assert_eq!(uut(needle, &[&1]), insert_from(0, &2));
        assert_eq!(uut(needle, &[&1, &2]), insert_from(0, &3));
        assert_eq!(uut(needle, &[&1, &2, &3]), insert_from(0, &4));
        assert_eq!(uut(needle, &[&1, &2, &3, &4]), matched(0));
        assert_eq!(uut(needle, &[&10, &1, &2, &3, &4]), matched(1));
        assert_eq!(uut(needle, &[&10, &10, &1, &2, &3, &4]), matched(2));
        //                        0   1   2   3  [4]
        assert_eq!(uut(needle, &[&1, &2, &3, &4, &1]), insert_from(4, &2));
        //                        0    1   [2]
        assert_eq!(uut(needle, &[&10, &10, &1, &2]), insert_from(2, &3));
    }
}
