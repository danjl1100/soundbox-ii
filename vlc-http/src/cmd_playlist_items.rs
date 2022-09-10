// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use crate::{
    vlc_responses::{PlaybackStatus, PlaylistInfo},
    Command,
};
use std::num::NonZeroUsize;
use tokio::sync::watch;

#[derive(Debug)]
/// Playlist Command data
pub struct Data {
    /// [`Url`](`url::Url`) of the playlist items.
    ///
    /// First element is the current / past item
    /// Remaining elements are the upcoming / next item(s)
    //TODO consider refactoring `Command::PlaylistSet` to accept one contiguous Vec<Url>?
    //     this requires expanding the high_command::playlist_set logic to accepting empty vec,
    //     which should probably clear all items *AFTER* the current playing item
    pub items: Vec<url::Url>,
    /// Maximum number of past-played items to keep in the history
    pub max_history_count: NonZeroUsize,
}

/// Sender type for Playlist Command [`Data`]
pub struct Sender {
    /// Sender for commanded Playlist Items
    pub urls_tx: watch::Sender<Data>,
    /// Receiver for signal to remove_current
    pub remove_rx: watch::Receiver<String>,
}
pub(crate) struct Receiver(Option<ReceiverInner>);
struct ReceiverInner {
    urls_rx: watch::Receiver<Data>,
    remove_tx: watch::Sender<String>,
    current_item_id: Option<u64>,
    playlist_item_urls: Vec<(u64, String)>,
}

pub(crate) fn channel(max_history_count: NonZeroUsize) -> (Sender, Receiver) {
    let (urls_tx, urls_rx) = watch::channel(Data {
        items: vec![],
        max_history_count,
    });
    let (remove_tx, remove_rx) = watch::channel(String::default());
    (
        Sender { urls_tx, remove_rx },
        Receiver(Some(ReceiverInner {
            urls_rx,
            remove_tx,
            current_item_id: None,
            playlist_item_urls: vec![],
        })),
    )
}
impl Receiver {
    /// Waits for a change, then returns the new value
    //NOTE not an `mpsc`, since needs only retain the latest value
    pub(crate) async fn recv_clone_cmd(
        &mut self,
    ) -> Option<Result<Command, watch::error::RecvError>> {
        let inner = self.0.as_mut()?;
        Some(inner.recv_clone_cmd().await)
    }
    pub(crate) fn notify_playback(&mut self, status: &PlaybackStatus) {
        self.notify_inner(|n| n.notify_playback(status));
    }
    pub(crate) fn notify_playlist(&mut self, playlist: &PlaylistInfo) {
        self.notify_inner(|n| n.notify_playlist(playlist));
    }
    fn notify_inner<F>(&mut self, notify_fn: F)
    where
        F: FnOnce(&mut ReceiverInner) -> Option<Destroy>,
    {
        if let Some(Destroy) = self.0.as_mut().and_then(notify_fn) {
            // Sender sliently destroyed the `remove_rx`, so abruptly end communications
            self.0.take();
        }
    }
}
struct Destroy;
impl ReceiverInner {
    /// Waits for a change, then returns the new value
    //NOTE not an `mpsc`, since needs only retain the latest value
    async fn recv_clone_cmd(&mut self) -> Result<Command, watch::error::RecvError> {
        self.urls_rx.changed().await?;
        let Data {
            items,
            max_history_count,
        } = &*self.urls_rx.borrow();
        let command = items.split_first().map_or_else(
            || Command::PlaybackStop,
            |(first, next)| {
                let first = first.clone();
                let next = next.to_vec();
                let max_history_count = *max_history_count;
                Command::PlaylistSet {
                    current_or_past_url: first,
                    next_urls: next,
                    max_history_count,
                }
            },
        );
        Ok(command)
    }
    fn notify_playback(&mut self, status: &PlaybackStatus) -> Option<Destroy> {
        self.current_item_id = status
            .information
            .as_ref()
            .and_then(|info| info.playlist_item_id);
        self.on_update()
    }
    fn notify_playlist(&mut self, playlist: &PlaylistInfo) -> Option<Destroy> {
        use std::str::FromStr;
        self.playlist_item_urls.clear();
        for item in &playlist.items {
            if let Ok(id) = u64::from_str(&item.id) {
                let url = item.url.clone();
                self.playlist_item_urls.push((id, url));
            }
        }
        self.on_update()
    }
    fn on_update(&mut self) -> Option<Destroy> {
        let current_item_id = self.current_item_id?;
        let cmd_current_url_str = self.urls_rx.borrow().items.first()?.to_string();
        let matched_url_str = self.playlist_item_urls.windows(2).find_map(|window| {
            let (_, previous_url_str) = &window[0];
            let (current_id, _) = &window[1];
            (*current_id == current_item_id && *previous_url_str == cmd_current_url_str)
                .then_some(previous_url_str)
        })?;
        let send_result = self.remove_tx.send(matched_url_str.to_string());
        if send_result.is_err() {
            // no more receivers, mark as not needed
            Some(Destroy)
        } else {
            None
        }
    }
}
