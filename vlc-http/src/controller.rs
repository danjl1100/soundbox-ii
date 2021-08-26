//! Controller for VLC-HTTP, with associated helper types

use crate::{Action, Credentials, Error, PlaybackStatus, PlaylistInfo, Query, ResultSender};
use shared::{Never, Shutdown};
use tokio::sync::{mpsc, watch};

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
                    let request = command.into();
                    let parse_result = response::try_parse(self.context.run(&request).await).await;
                    let send_result = match parse_result {
                        Ok(typed) => {
                            self.update_status(typed);
                            Ok(())
                        }
                        Err(e) => Err(e),
                    };
                    Self::send_result(send_result, result_tx);
                }
                Action::QueryPlaybackStatus(result_tx) => {
                    let request = Query::PlaybackStatus.into();
                    let result = response::try_parse(self.context.run(&request).await).await;
                    let cloned_result = match result {
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
                    let request = Query::PlaylistInfo.into();
                    let result = response::try_parse(self.context.run(&request).await).await;
                    let cloned_result = match result {
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
            }
        }
    }
    fn send_result<T>(result: Result<T, Error>, result_tx: ResultSender<T>)
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
            response::Typed::Art => {}
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

mod response {
    use crate::{Error, PlaybackStatus, PlaylistInfo, RequestType};
    #[derive(Debug)]
    #[allow(clippy::large_enum_variant)]
    pub(crate) enum Typed {
        Art,
        Playback(PlaybackStatus),
        Playlist(PlaylistInfo),
    }

    pub(crate) async fn try_parse(
        response: Result<(RequestType, hyper::Response<hyper::Body>), hyper::Error>,
    ) -> Result<Typed, Error> {
        match response {
            Ok((RequestType::Art, _)) => todo!(),
            Ok((RequestType::Status, response)) => {
                parse_typed(response, PlaybackStatus::from_slice)
                    .await
                    .map(Typed::Playback)
            }
            Ok((RequestType::Playlist, response)) => {
                parse_typed(response, PlaylistInfo::from_slice)
                    .await
                    .map(Typed::Playlist)
            }
            Err(e) => Err(e.into()),
        }
    }
    pub(crate) async fn parse_typed<F, T, E>(
        response: hyper::Response<hyper::Body>,
        map_fn: F,
    ) -> Result<T, Error>
    where
        F: FnOnce(&[u8]) -> Result<T, E>,
        Error: From<E>,
    {
        hyper::body::to_bytes(response.into_body())
            .await
            .map_err(|err| err.into())
            .and_then(|bytes| Ok(map_fn(&bytes)?))
    }
    //TODO: deleteme
    pub(crate) async fn parse_body_json<F, T, E>(
        result: Result<hyper::Response<hyper::Body>, hyper::Error>,
        map_fn: F,
    ) -> Result<T, Error>
    where
        F: FnOnce(&[u8]) -> Result<T, E>,
        Error: From<E>,
    {
        match result {
            Ok(response) => hyper::body::to_bytes(response.into_body())
                .await
                .map_err(|err| err.into())
                .and_then(|bytes| Ok(map_fn(&bytes)?)),
            Err(err) => Err(err.into()),
        }
    }
}

use context::Context;
mod context {
    use crate::{
        auth::Credentials,
        command::{RequestIntent, RequestType},
        request::RequestInfo,
    };
    use hyper::{
        body::Body, client::Builder as ClientBuilder, Client as HyperClient,
        Request as HyperRequest,
    };
    type Client = HyperClient<hyper::client::connect::HttpConnector, Body>;
    type Request = HyperRequest<Body>;

    /// Execution context for [`RequestIntent`]s
    pub(crate) struct Context(Client, Credentials);
    impl Context {
        pub fn new(credentials: Credentials) -> Self {
            let client = ClientBuilder::default().build_http();
            Self(client, credentials)
        }
        pub async fn run<'a, 'b>(
            &self,
            request_intent: &RequestIntent<'a, 'b>,
        ) -> Result<(RequestType, hyper::Response<Body>), hyper::Error> {
            let request_type = request_intent.get_type();
            let request_info = RequestInfo::from(request_intent);
            Ok((request_type, self.run_retry_loop(request_info).await?))
        }
        async fn run_retry_loop(
            &self,
            request: RequestInfo,
        ) -> Result<hyper::Response<Body>, hyper::Error> {
            use backoff::ExponentialBackoff;
            use tokio::time::Duration;
            let backoff_config = ExponentialBackoff {
                current_interval: Duration::from_millis(50),
                initial_interval: Duration::from_millis(50),
                multiplier: 4.0,
                max_interval: Duration::from_secs(2),
                max_elapsed_time: Some(Duration::from_secs(10)),
                ..ExponentialBackoff::default()
            };
            backoff::future::retry(backoff_config, || async {
                let request = self.request_from(request.clone()); //TODO: avoid expensive clone?
                Ok(self.0.request(request).await?)
            })
            .await
        }
        fn request_from(&self, info: RequestInfo) -> Request {
            let RequestInfo {
                path_and_query,
                method,
            } = info;
            let uri = self
                .1
                .uri_builder()
                .path_and_query(path_and_query)
                .build()
                .expect("internally-generated URI is valid");
            self.1
                .request_builder()
                .uri(uri)
                .method(method)
                .body(Body::empty())
                .expect("internally-generated URI and Method is valid")
        }
    }
}
