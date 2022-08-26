// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Controller for VLC-HTTP, with associated helper types

use std::convert::TryFrom;

use crate::{
    command::{ArtRequestIntent, HighCommand, RequestIntent},
    http_client::{response, Context},
    rules::Rules,
    Action, Authorization, Error, LowCommand, PlaybackStatus, PlaylistInfo, Query, RepeatMode,
};
use shared::{Never, Shutdown};
use tokio::sync::{mpsc, oneshot, watch};

mod high_converter;
mod rate;
mod ret;

#[cfg(test)]
mod tests;

/// Channels for interfacing with a [`Controller`]
pub struct Channels {
    /// Sender for [`Action`]s
    pub action_tx: mpsc::Sender<Action>,
    /// Receiver for [`PlaybackStatus`]
    pub playback_status_rx: watch::Receiver<Option<PlaybackStatus>>,
    /// Receiver for [`PlaylistInfo`]
    pub playlist_info_rx: watch::Receiver<Option<PlaylistInfo>>,
}

/// Control interface for VLC-HTTP
///
/// # Example
///
/// ```
/// use vlc_http::{Authorization, Credentials, Controller};
///
/// let auth = Authorization::try_from(Credentials {
///     password: "1234".to_string(),
///     host: "localhost".to_string(),
///     port: 22,
/// }).expect("valid credentials");
/// let (controller, _channels) = Controller::new(auth);
///
/// let async_task = controller.run();
/// // Then, actually spawn the task, e.g:
/// //   tokio::spawn(async_task)
/// ```
pub struct Controller {
    action_rx: mpsc::Receiver<Action>,
    playback_status_tx: watch::Sender<Option<PlaybackStatus>>,
    playlist_info_tx: watch::Sender<Option<PlaylistInfo>>,
    context: Context,
    rules: Rules,
    rate_limit_action_rx: rate::Limiter,
}
impl Controller {
    const RATE_LIMIT_MS: u32 = 90;
    /// Creates a [`Controller`] with the associated control [`Channels`]
    pub fn new(authorization: Authorization) -> (Self, Channels) {
        // Channels
        let (action_tx, action_rx) = mpsc::channel(1);
        let (playback_status_tx, playback_status_rx) = watch::channel(None);
        let (playlist_info_tx, playlist_info_rx) = watch::channel(None);
        let channels = Channels {
            action_tx,
            playback_status_rx,
            playlist_info_rx,
        };
        // Controller
        let controller = {
            let context = Context::new(authorization);
            let rules = Rules::new();
            Self {
                action_rx,
                playback_status_tx,
                playlist_info_tx,
                context,
                rules,
                rate_limit_action_rx: rate::Limiter::new(Self::RATE_LIMIT_MS),
            }
        };
        (controller, channels)
    }
}
impl Controller {
    /// Executes the all received actions
    ///
    /// # Errors
    /// Returns a [`Shutdown`] error when no [`Action`] senders remain
    ///
    pub async fn run(mut self) -> Result<Never, Shutdown> {
        loop {
            // decide action
            let decision_time = shared::time_now();
            // TODO add tracing
            // dbg!(decision_time);
            let action = {
                tokio::select! {
                    biased; // prioritize External over Internal actions
                    external_action = self.action_rx.recv() => {
                        let external_action = external_action.ok_or(Shutdown)?;
                        // rate-limit commands only (allow rule-based actions)
                        self.rate_limit_action_rx.enter().await;
                        external_action
                    }
                    Some(internal_action) = self.rules.next_action(decision_time) => {
                        internal_action
                    }
                    else => {
                        return Err(Shutdown);
                    }
                }
            };
            // run action
            println!("VLC-RUN {}", &action);
            self.run_action(action).await;
        }
    }
    async fn run_action(&mut self, action: Action) {
        match action {
            Action::Command(command, result_tx) => {
                let send_result = match LowCommand::try_from(command) {
                    Ok(low_command) => self.run_low_command(low_command).await,
                    Err(high_command) => high_converter::State::from(high_command).run(self).await,
                };
                Self::send_result(send_result, result_tx);
            }
            Action::QueryPlaybackStatus(result_tx) => {
                // optionally send result to `result_tx` (if provided)
                if let Some(tx) = result_tx {
                    let result = self.run_query_playback_status::<ret::Some>().await;
                    Self::send_result(result, tx);
                } else {
                    match self.run_query_playback_status::<ret::None>().await {
                        Ok(()) => {}
                        Err(e) => {
                            dbg!(e);
                        }
                    }
                }
            }
            Action::QueryPlaylistInfo(result_tx) => {
                // optionally send result to `result_tx` (if provided)
                if let Some(tx) = result_tx {
                    let result = self.run_query_playlist_info::<ret::Some>().await;
                    Self::send_result(result, tx);
                } else {
                    match self.run_query_playlist_info::<ret::None>().await {
                        Ok(()) => {}
                        Err(e) => {
                            dbg!(e);
                        }
                    }
                }
            }
            Action::QueryArt(result_tx) => {
                let request = ArtRequestIntent { id: None };
                let result = response::try_parse(self.context.run(&request).await).await;
                Self::send_result(result, result_tx);
            }
        }
    }
}

impl Controller {
    async fn run_low_command(&mut self, low_command: LowCommand) -> Result<(), Error> {
        // notify rules
        {
            let action_time = shared::time_now();
            self.rules.notify_command(action_time, &low_command);
        }
        // run action
        {
            let result = self.run_and_parse_text(low_command).await;
            result.map(|typed| {
                self.update_status(typed);
            })
        }
    }
    async fn run_query_playback_status<T>(&mut self) -> Result<T::Return, Error>
    where
        T: ret::Returner<PlaybackStatus>,
    {
        self.run_and_parse_text(Query::PlaybackStatus)
            .await
            .map(|response| match response {
                response::Typed::Playback(playback) => playback,
                response::Typed::Playlist(_) => {
                    //TODO change from `should` to `enforced by type system`
                    unreachable!("PlaybackRequest should be type Playback")
                }
            })
            .map(|playback| {
                T::apply_with(playback, |p| {
                    self.update_status(p.into());
                })
            })
    }
    async fn run_query_playlist_info<T>(&mut self) -> Result<T::Return, Error>
    where
        T: ret::Returner<PlaylistInfo>,
    {
        self.run_and_parse_text(Query::PlaylistInfo)
            .await
            .map(|response| match response {
                response::Typed::Playlist(playlist) => playlist,
                response::Typed::Playback(_) => {
                    //TODO change from `should` to `enforced by type system`
                    /* use something like this inside vlc_http/src/http_client.rs
                    trait ResponseType
                    where
                        Self: Sized,
                    {
                        type Error: Into<Error>;
                        fn from_slice(bytes: &[u8], received_time: shared::Time) -> Result<Self, Self::Error>;
                    }
                    */
                    unreachable!("PlaylistRequest should be type Playlist")
                }
            })
            .map(|playlist| {
                T::apply_with(playlist, |p| {
                    self.update_status(p.into());
                })
            })
    }
    async fn run_and_parse_text<'a, 'b, T>(&mut self, request: T) -> Result<response::Typed, Error>
    where
        RequestIntent<'a, 'b>: From<T>,
    {
        let request = RequestIntent::from(request);
        let req_type = request.get_type();
        let result = self.context.run(&request).await;
        response::try_parse_body_text(result.map(|r| (req_type, r))).await
    }
    fn send_result<T>(result: T, result_tx: oneshot::Sender<T>)
    where
        T: std::fmt::Debug,
    {
        let send_result = result_tx.send(result);
        if let Err(send_err) = send_result {
            println!("WARNING: result_tx send error: {:?}", send_err);
        }
    }
    fn update_status(&mut self, typed_response: response::Typed) {
        let Self {
            rules,
            playback_status_tx,
            playlist_info_tx,
            ..
        } = self;
        match typed_response {
            response::Typed::Playback(playback) => {
                send_if_changed(playback, playback_status_tx, move |p| {
                    rules.notify_playback(p);
                });
            }
            response::Typed::Playlist(playlist) => {
                send_if_changed(playlist, playlist_info_tx, move |p| {
                    rules.notify_playlist(p);
                });
            }
        }
    }
}
fn send_if_changed<T, F>(new_value: T, sender: &mut watch::Sender<Option<T>>, notify_fn: F)
where
    T: PartialEq + Clone,
    F: FnOnce(&T),
{
    match &*sender.borrow() {
        Some(prev) if *prev == new_value => {
            // identical, no change to publish
            return;
        }
        _ => {}
    }
    // changed!
    notify_fn(&new_value);
    if !sender.is_closed() {
        let _ignore_err = sender.send(Some(new_value));
    }
}
