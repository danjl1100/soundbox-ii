//! Controller for VLC-HTTP, with associated helper types

use crate::{
    command::{ArtRequestIntent, RequestIntent},
    http_client::{response, Context},
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
        Controller {
            action_rx,
            playback_status_tx,
            playlist_info_tx,
            context,
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
            let action = self.action_rx.recv().await.ok_or(Shutdown)?;
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
        match typed_response {
            response::Typed::Playback(playback) => {
                send_if_changed(&mut self.playback_status_tx, playback);
            }
            response::Typed::Playlist(playlist) => {
                send_if_changed(&mut self.playlist_info_tx, playlist);
            }
        }
    }
}
fn send_if_changed<T: PartialEq + Clone>(sender: &mut watch::Sender<Option<T>>, new_value: T) {
    if !sender.is_closed() {
        let mut option = sender.borrow().clone();
        let should_send = replace_option_changed(&mut option, new_value);
        if should_send {
            let _ignore_err = sender.send(option);
        }
    }
}
fn replace_option_changed<T: PartialEq>(option: &mut Option<T>, new_value: T) -> bool {
    let identical = matches!(option, Some(prev) if *prev == new_value);
    let changed = !identical;
    *option = Some(new_value);
    changed
}
