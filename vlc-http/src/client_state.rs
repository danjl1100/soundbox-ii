// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Types to track the state of a specific VLC instance

use self::sequenced::Sequenced;
use crate::{response, Response};

pub(crate) use sequenced::Sequence;
mod sequenced;

/// Tracks the state of a specific VLC instance
#[derive(Clone, Debug)]
#[must_use]
pub struct ClientState {
    // NOTE: do not necessarily store the entire playlist... it can easily be re-queried
    playlist_info: Sequenced<response::PlaylistInfo>,
    playback_status: Sequenced<()>,
}

impl ClientState {
    /// Returns an empty state for a specific VLC client
    pub fn new() -> Self {
        let builder = Sequenced::builder();
        Self {
            playlist_info: builder.next_default(),
            playback_status: builder.next_default(),
        }
    }

    // TODO is helper needed? likely... Response is opaque (other than for `Debug`)
    // pub fn update_from_response<R>(&mut self, response: R) -> Result<(), response::ParseError>
    // where
    //     R: std::io::Read,
    // {
    //     let response = Response::from_reader(response)?;
    //     self.update(response);
    //     Ok(())
    // }

    /// Updates the state for the specified [`Response`]
    ///
    /// This allows [`Action`](`crate::Action`)s to progress to return a result, or a new
    /// [`Endpoint`](`crate::Endpoint`)
    pub fn update(&mut self, response: Response) {
        match response.inner {
            crate::response::ResponseInner::PlaylistInfo(new) => {
                let _prev = self.playlist_info.replace(new);
            }
            crate::response::ResponseInner::PlaybackStatus(_) => {
                // TODO
                self.playback_status.modify(|()| ());
                drop(response);
            }
        }
    }

    pub(crate) fn playlist_info(&self) -> &Sequenced<response::PlaylistInfo> {
        &self.playlist_info
    }
}
impl Default for ClientState {
    fn default() -> Self {
        Self::new()
    }
}
