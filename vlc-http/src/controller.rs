// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Controller for VLC-HTTP, with associated helper types

use std::convert::TryFrom;

use crate::{
    command::{ArtRequestIntent, HighCommand, RequestIntent},
    http_client::{response, Context},
    rules::Rules,
    Action, Credentials, Error, LowCommand, PlaybackStatus, PlaylistInfo, Query,
};
use shared::{Never, Shutdown};
use tokio::sync::{mpsc, oneshot, watch};

/// Configuration for [`Controller`]
pub struct Config {
    /// Receiver for [`Action`]s
    pub action_rx: mpsc::Receiver<Action>,
    /// Sender for [`PlaybackStatus`]
    pub playback_status_tx: watch::Sender<Option<PlaybackStatus>>,
    /// Sender for [`PlaylistInfo`]
    pub playlist_info_tx: watch::Sender<Option<PlaylistInfo>>,
    /// Credentials
    pub credentials: Credentials,
}
/// Control interface for VLC-HTTP
pub struct Controller {
    action_rx: mpsc::Receiver<Action>,
    playback_status_tx: watch::Sender<Option<PlaybackStatus>>,
    playlist_info_tx: watch::Sender<Option<PlaylistInfo>>,
    context: Context,
    rules: Rules,
    rate_limit_action_rx: RateLimiter,
}
impl Controller {
    /// Creates a [`Controller`] from the specified [`Config`]
    pub fn new(config: Config) -> Self {
        const RATE_LIMIT_MS: u32 = 90;
        let Config {
            action_rx,
            playback_status_tx,
            playlist_info_tx,
            credentials,
        } = config;
        let context = Context::new(credentials);
        let rules = Rules::new();
        Self {
            action_rx,
            playback_status_tx,
            playlist_info_tx,
            context,
            rules,
            rate_limit_action_rx: RateLimiter::new(RATE_LIMIT_MS),
        }
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
                    Ok(low_command) => self.run_command(low_command).await,
                    Err(high_command) => self.run_high_command(high_command).await,
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
/// Helper zero-sized types to facilitate optionally returning a value
/// This allows cloning only when needed, as the clone only occurs when [`Some`] value is requested.
mod ret {
    mod private {
        pub trait Sealed {}
        impl Sealed for super::Some {}
        impl Sealed for super::None {}
    }
    pub trait Returner<T>: private::Sealed {
        type Return;
        /// Applies the data to the `observer`, then returns
        fn apply_with<F>(t: T, observer: F) -> Self::Return
        where
            F: FnOnce(T);
        // TODO remove unused
        // fn apply(t: T) -> Self::Return;
    }
    /// Some return data is requested
    pub enum Some {}
    impl<T: Clone> Returner<T> for Some {
        type Return = T;
        fn apply_with<F>(t: T, observer: F) -> T
        where
            F: FnOnce(T),
        {
            observer(t.clone());
            t
        }
        // fn apply(t: T) -> T {
        //     t
        // }
    }
    /// Return data is not needed
    pub enum None {}
    impl<T> Returner<T> for None {
        type Return = ();
        fn apply_with<F>(t: T, observer: F)
        where
            F: FnOnce(T),
        {
            observer(t);
        }
        // fn apply(_: T) {}
    }
}

shared::wrapper_enum! {
    enum LowAction {
        Command(LowCommand),
        { impl None for }
        QueryPlaybackStatus,
        QueryPlaylistInfo,
    }
}

impl Controller {
    fn get_next_low_action_for(&self, command: &HighCommand) -> Option<LowAction> {
        Some(match command {
            HighCommand::PlaybackMode { repeat, random } => {
                match &*self.playback_status_tx.borrow() {
                    None => LowAction::QueryPlaybackStatus,
                    Some(s) => match () {
                        _ if s.is_loop_all != repeat.is_loop_all() => {
                            LowCommand::ToggleLoopAll.into()
                        }
                        _ if s.is_repeat_one != repeat.is_repeat_one() => {
                            LowCommand::ToggleRepeatOne.into()
                        }
                        _ if s.is_random != *random => LowCommand::ToggleRandom.into(),
                        _ => {
                            // base case, matches desired state
                            return None;
                        }
                    },
                }
            }
        })
    }
    async fn run_high_command(&mut self, high_command: HighCommand) -> Result<(), Error> {
        while let Some(low_action) = self.get_next_low_action_for(&high_command) {
            match low_action {
                LowAction::Command(low_command) => {
                    self.run_command(low_command).await?;
                }
                LowAction::QueryPlaybackStatus => {
                    self.run_query_playback_status::<ret::None>().await?;
                }
                LowAction::QueryPlaylistInfo => {
                    self.run_query_playlist_info::<ret::None>().await?;
                }
            }
        }
        Ok(())
    }
    async fn run_command(&mut self, low_command: LowCommand) -> Result<(), Error> {
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

struct RateLimiter {
    interval: shared::TimeDifference,
    last_time: Option<shared::Time>,
}
impl RateLimiter {
    fn new(interval_millis: u32) -> Self {
        Self {
            interval: shared::TimeDifference::milliseconds(interval_millis.into()),
            last_time: None,
        }
    }
    async fn enter(&mut self) {
        let now = shared::time_now();
        if let Some(last_act_time) = self.last_time {
            let since_last_act = now - last_act_time;
            let remaining_delay = self.interval - since_last_act;
            if let Ok(delay) = remaining_delay.to_std() {
                dbg!("waiting {:?}", delay);
                tokio::time::sleep(delay).await;
            }
        }
        self.last_time = Some(now);
    }
}

#[cfg(test)]
mod tests {
    use super::{Action, PlaybackStatus, PlaylistInfo, Rules};
    use shared::time_from_secs as time;
    use shared::time_now;

    #[tokio::test]
    async fn rules_initialize_status() {
        let start = time_now();
        let mut rules = Rules::new();
        assert_eq!(
            rules.next_action(time(0)).await,
            Some(Action::fetch_playback_status())
        );
        rules.notify_playback(&PlaybackStatus::default());
        assert_eq!(
            rules.next_action(time(0)).await,
            Some(Action::fetch_playlist_info())
        );
        rules.notify_playlist(&PlaylistInfo::default());
        assert_eq!(rules.next_action(time(0)).await, None); // TODO: remove me when the ongoing fetch action is added
        let end = time_now();
        let duration = end - start;
        assert!(duration.num_seconds() < 2);
    }
}
