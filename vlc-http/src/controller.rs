//! Controller for VLC-HTTP, with associated helper types

use crate::{
    command::{ArtRequestIntent, RequestIntent},
    http_client::{response, Context},
    rules::Rules,
    Action, Credentials, Error, PlaybackStatus, PlaylistInfo, Query,
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
}
impl Config {
    /// Creates a [`Controller`] form the specified [`Config`]
    pub fn build(self) -> Controller {
        let Self {
            action_rx,
            playback_status_tx,
            playlist_info_tx,
            credentials,
        } = self;
        let context = Context::new(credentials);
        let rules = Rules::new();
        Controller {
            action_rx,
            playback_status_tx,
            playlist_info_tx,
            context,
            rules,
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
        // let mut last_act_time = None;
        loop {
            // decide action
            let decision_time = shared::time_now();
            // TODO add tracing
            // dbg!(decision_time);
            let action = {
                tokio::select! {
                    biased; // prioritize External over Internal actions
                    external_action = self.action_rx.recv() => {
                        external_action.ok_or(Shutdown)?
                    }
                    Some(internal_action) = self.rules.next_action(decision_time) => {
                        internal_action
                    }
                    else => {
                        return Err(Shutdown);
                    }
                }
            };
            // TODO: is this worth it?   ideally need to rate-limit commands, but allow fetches unimpeded
            // // sleep (rate limiting)
            // if let Some(last_act_time) = last_act_time {
            //     const RATE_LIMIT_MS: u32 = 90;
            //     let since_last_act: shared::TimeDifference = shared::time_now() - last_act_time;
            //     let remaining_delay = shared::TimeDifference::milliseconds(RATE_LIMIT_MS.into()) - since_last_act;
            //     if let Ok(delay) = remaining_delay.to_std() {
            //         println!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!");
            //         println!("WAITING!!!!!!!!!!!!!!!! {:?}", delay);
            //         println!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!");
            //         tokio::time::sleep(delay).await;
            //     }
            // }
            // run action
            let action_time = shared::time_now();
            if let Action::Command(command, _) = &action {
                self.rules.notify_command(action_time, command);
            }
            println!("VLC-RUN {}", &action);
            self.run_action(action).await;
            // last_act_time = Some(action_time);
        }
    }
    async fn run_action(&mut self, action: Action) {
        match action {
            Action::Command(command, result_tx) => {
                let parse_result = self.run_and_parse_text(command).await;
                let send_result = parse_result.map(|typed| {
                    self.update_status(typed);
                });
                Self::send_result(send_result, result_tx);
            }
            Action::QueryPlaybackStatus(result_tx) => {
                let parse_result = self.run_and_parse_text(Query::PlaybackStatus).await;
                let cloned_result = match parse_result {
                    Ok(response::Typed::Playback(playback)) => {
                        // (optional clone)
                        let cloned = result_tx.map(|tx| (Ok(playback.clone()), tx));
                        // send status
                        self.update_status(response::Typed::Playback(playback));
                        cloned
                    }
                    Err(e) => result_tx.map(|tx| (Err(e), tx)),
                    Ok(_) => unreachable!("PlaybackRequest should be type Playback"),
                };
                if let Some((result, tx)) = cloned_result {
                    Self::send_result(result, tx);
                }
            }
            Action::QueryPlaylistInfo(result_tx) => {
                let parse_result = self.run_and_parse_text(Query::PlaylistInfo).await;
                let cloned_result = match parse_result {
                    Ok(response::Typed::Playlist(playlist)) => {
                        // (optional clone)
                        let cloned = result_tx.map(|tx| (Ok(playlist.clone()), tx));
                        // send status
                        self.update_status(response::Typed::Playlist(playlist));
                        cloned
                    }
                    Err(e) => result_tx.map(|tx| (Err(e), tx)),
                    Ok(_) => unreachable!("PlaylistInfo should be type Playlist"),
                };
                if let Some((result, tx)) = cloned_result {
                    Self::send_result(result, tx);
                }
            }
            Action::QueryArt(result_tx) => {
                let request = ArtRequestIntent { id: None };
                let result = response::try_parse(self.context.run(&request).await).await;
                Self::send_result(result, result_tx);
            }
        }
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
