use crate::{Action, Credentials, Error, PlaybackStatus, PlaylistInfo, Query, ResultSender};
use tokio::sync::{mpsc, watch};

/// Control interface for VLC-HTTP
pub struct Controller {
    /// Receiver for [`Action`]s
    pub action_rx: mpsc::Receiver<Action>,
    /// Sender for [`PlaybackStatus`]
    pub playback_status_tx: watch::Sender<Option<PlaybackStatus>>,
    /// Sender for [`PlaylistInfo`]
    pub playlist_info_tx: watch::Sender<Option<PlaylistInfo>>,
}
impl Controller {
    /// Executes the specified actions
    pub async fn run(mut self, credentials: Credentials) {
        let context = Context::new(credentials);
        while let Some(action) = self.action_rx.recv().await {
            match action {
                Action::Command(command, result_tx) => {
                    let request = command.into();
                    let result = context
                        .run(&request)
                        .await
                        .map(response::parse)
                        .map_err(|e| e.into());
                    let send_result = match result {
                        Ok(response_fut) => match response_fut.await {
                            Ok(typed) => {
                                self.update_status(typed);
                                Ok(())
                            }
                            Err(e) => Err(e),
                        },
                        Err(e) => Err(e),
                    };
                    Self::send_result(send_result, result_tx);
                }
                Action::QueryPlaybackStatus(result_tx) => {
                    let request = Query::PlaybackStatus.into();
                    let result = context
                        .run(&request)
                        .await
                        .map(response::parse)
                        .map_err(|e| e.into());
                    let send_result = match result {
                        Ok(response_fut) => match response_fut.await {
                            Ok(response::Typed::Playback(playback)) => {
                                let send_result = Ok(playback.clone());
                                self.update_status(response::Typed::Playback(playback));
                                send_result
                            }
                            Ok(_) => unreachable!("PlaybackRequest should be Playback"),
                            Err(e) => Err(e),
                        },
                        Err(e) => Err(e),
                    };
                    if let Some(result_tx) = result_tx {
                        Self::send_result(send_result, result_tx);
                    }
                }
                Action::QueryPlaylistInfo(result_tx) => {
                    let request = Query::PlaylistInfo.into();
                    let response = context.run(&request).await.map(|(_, result)| result); //TODO remove me
                    let result =
                        response::parse_body_json(response, PlaylistInfo::from_slice).await; //TODO remove me
                    if let Ok(playlist) = &result {
                        self.update_status(response::Typed::Playlist((*playlist).clone()));
                    }
                    if let Some(result_tx) = result_tx {
                        Self::send_result(result, result_tx);
                    }
                }
            };
        }
        println!("vlc_http::run() - context ended!");
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

    pub(crate) async fn parse(
        (res_type, response): (RequestType, hyper::Response<hyper::Body>),
    ) -> Result<Typed, Error> {
        match res_type {
            RequestType::Art => todo!(),
            RequestType::Status => parse_typed(response, PlaybackStatus::from_slice)
                .await
                .map(Typed::Playback),
            RequestType::Playlist => parse_typed(response, PlaylistInfo::from_slice)
                .await
                .map(Typed::Playlist),
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
            use backoff::{future::retry, ExponentialBackoff};
            use tokio::time::Duration;
            let backoff_config = ExponentialBackoff {
                current_interval: Duration::from_millis(50),
                initial_interval: Duration::from_millis(50),
                multiplier: 4.0,
                max_interval: Duration::from_secs(2),
                max_elapsed_time: Some(Duration::from_secs(10)),
                ..ExponentialBackoff::default()
            };
            retry(backoff_config, || async {
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
