// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
#![allow(opaque_hidden_inferred_bound)] // warp crate-private module cannot be referenced in the
                                        // `impl Reply` bounds
pub(crate) use filter::root as filter;
mod filter {
    use http::uri::Uri;
    use std::path::PathBuf;
    use tokio::sync::mpsc;
    use warp::{Filter, Reply};

    pub(crate) fn root(
        config: super::web_socket::Config,
        assets_dir: PathBuf,
    ) -> impl Filter<Extract = impl Reply, Error = warp::Rejection> + Clone {
        let vlc_tx = config.vlc_tx.clone();

        root_redirect()
            .or(api_v1::root(vlc_tx))
            .or(static_files(assets_dir))
            .or(super::web_socket::filter(config))
    }

    fn with_sender<T: Send + Sync>(
        sender: mpsc::Sender<T>,
    ) -> impl Filter<Extract = (mpsc::Sender<T>,), Error = std::convert::Infallible> + Clone {
        warp::any().map(move || sender.clone())
    }

    fn root_redirect() -> impl Filter<Extract = impl Reply, Error = warp::Rejection> + Clone {
        // NOTE: temporary, in case we change it later
        warp::path::end().map(|| warp::redirect::temporary(Uri::from_static("/app/")))
    }

    fn static_files(
        assets_dir: PathBuf,
    ) -> impl Filter<Extract = impl Reply, Error = warp::Rejection> + Clone {
        warp::get().and(warp::path("app")).and(
            warp::fs::dir(assets_dir.clone()) // load specific file
                .or(warp::path::tail() // OR, ignore tail
                    .and(warp::fs::dir(assets_dir)) // loads index.html (pretend requested `/app/`)
                    .map(|_tail, file| file)),
        )
    }

    mod api_v1 {
        use super::with_sender;
        use warp::{Filter, Reply};

        type VlcTx = tokio::sync::mpsc::Sender<vlc_http::Action>;
        type Response = hyper::Response<hyper::Body>;

        pub fn root(
            vlc_tx: VlcTx,
        ) -> impl Filter<Extract = impl Reply, Error = warp::Rejection> + Clone {
            warp::path("v1") //
                .and(warp::get()) //
                .and(album_art(vlc_tx))
        }

        fn album_art(
            vlc_tx: VlcTx,
        ) -> impl Filter<Extract = (Response,), Error = warp::Rejection> + Clone {
            warp::path("art") //
                .and(with_sender(vlc_tx)) //
                .and_then(|vlc_tx| async move {
                    let response = query_album_art(vlc_tx)
                        .await
                        .map_or_else(build_response, |r| r);
                    Ok::<_, std::convert::Infallible>(response)
                })
        }
        async fn query_album_art(vlc_tx: VlcTx) -> Result<Response, (String, hyper::StatusCode)> {
            fn internal_err<E: std::fmt::Display>(err: E) -> (String, hyper::StatusCode) {
                let text = format!("internal error with vlc_http art module: \"{err}\"");
                (text, hyper::StatusCode::INTERNAL_SERVER_ERROR)
            }
            #[allow(clippy::needless_pass_by_value)] // helpful, to clarify Result<_, String> signature
            fn vlc_error(err_message: String) -> (String, hyper::StatusCode) {
                let text = format!(
                    r#"VLC reported error: "{}" (missing album art?)"#,
                    err_message
                );
                (text, hyper::StatusCode::NOT_FOUND)
            }
            // send Action
            let (action, result_rx) = vlc_http::Action::query_art();
            vlc_tx.send(action).await.map_err(internal_err)?;
            // poll result
            let result = result_rx.await.map_err(internal_err)?;
            // parse result
            let response: Result<Response, String> = result.map_err(internal_err)?;
            response.map_err(vlc_error)
        }
        fn build_response(
            (text, status_code): (String, hyper::StatusCode),
        ) -> hyper::Response<hyper::Body> {
            let mut response = text.into_response();
            *response.status_mut() = status_code;
            response
        }
    }
}

pub(crate) use web_socket::Config;
mod web_socket {
    use crate::WebSourceChanged;
    use futures::{SinkExt, StreamExt};
    use shared::{ClientRequest, ServerResponse};
    use tokio::sync::{mpsc, watch};
    use vlc_http::{Action, IntoAction, PlaybackStatus};
    use warp::ws::Message;
    use warp::{Filter, Reply};

    #[derive(Clone)]
    pub(crate) struct Config {
        pub vlc_tx: mpsc::Sender<Action>,
        pub playback_status_rx: watch::Receiver<Option<PlaybackStatus>>,
        // pub playlist_info_rx: watch::Receiver<Option<PlaylistInfo>>, //TODO use this field, or remove it!
        pub reload_rx: watch::Receiver<WebSourceChanged>,
    }
    pub(crate) fn filter(
        config: Config,
    ) -> impl Filter<Extract = impl Reply, Error = warp::Rejection> + Clone {
        warp::path("ws")
            .and(warp::ws())
            .map(move |ws: warp::ws::Ws| {
                let config = config.clone();
                ws.on_upgrade(|websocket| ClientHandler { websocket, config }.run_ignore_err())
            })
    }
    struct ClientHandler {
        websocket: warp::ws::WebSocket,
        config: Config,
    }
    impl ClientHandler {
        async fn run_ignore_err(self) {
            let _ = self.run().await;
        }
        async fn run(mut self) -> Result<(), ()> {
            {
                let _ignore_initial_value = self.config.reload_rx.borrow_and_update();
            }
            self.send_response(ServerResponse::Heartbeat).await?;
            loop {
                let send_message = tokio::select! {
                    Ok(()) = self.config.reload_rx.changed() => {
                        Some(ServerResponse::ClientCodeChanged)
                    }
                    Some(body) = self.websocket.next() => {
                        let message = match body {
                            Ok(msg) => msg,
                            Err(e) => {
                                eprintln!("Error reading message on websocket: {e}");
                                break;
                            }
                        };
                        // tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                        self.handle_message(message).await
                    }
                    Ok(_) = self.config.playback_status_rx.changed() => {
                        let now = shared::time_now();
                        let playback = (*self.config.playback_status_rx.borrow())
                            .as_ref()
                            .map(|s| vlc_http::PlaybackStatus::clone_to_shared(s, now))
                            .map(ServerResponse::from);
                        playback
                    }
                    _ = tokio::time::sleep(tokio::time::Duration::from_secs(30)) => {
                        Some(ServerResponse::Heartbeat)
                    }
                    else => {
                        break;
                    }
                };
                if let Some(message) = send_message {
                    self.send_response(message).await?;
                }
            }
            Ok(())
        }
        async fn handle_message(&mut self, message: Message) -> Option<ServerResponse> {
            // Skip any non-Text messages...
            let msg = if let Ok(s) = message.to_str() {
                s
            } else {
                println!("ping-pong");
                return None;
            };
            let response = match serde_json::from_str(msg) {
                Ok(request) => self.process_request(request).await,
                Err(err) => {
                    dbg!(&err);
                    ServerResponse::from_result(Err(err))
                }
            };
            Some(response)
        }
        async fn send_response(&mut self, message: ServerResponse) -> Result<(), ()> {
            let response_str = serde_json::to_string(&message).map_err(|_| ())?;

            self.websocket
                .send(Message::text(response_str))
                .await
                .map(|_| ())
                .map_err(|_| ())
        }
        async fn process_request(&mut self, request: ClientRequest) -> ServerResponse {
            match request {
                ClientRequest::Heartbeat => ServerResponse::Heartbeat,
                ClientRequest::Command(command) => {
                    let command = vlc_http::Command::from(command);
                    self.process_action(command).await
                }
            }
        }
        async fn process_action<T>(&mut self, action: T) -> ServerResponse
        where
            T: IntoAction<Output = ()>,
        {
            let (action, result_rx) = action.to_action_rx();
            let send_result = self.config.vlc_tx.send(action).await;
            match send_result {
                Ok(()) => {
                    let recv_result = result_rx.await;
                    match recv_result {
                        Ok(result) => ServerResponse::from_result(result),
                        Err(recv_err) => ServerResponse::from_result(Err(recv_err)),
                    }
                }
                Err(send_error) => {
                    dbg!(&send_error);
                    ServerResponse::from_result(Err(send_error))
                }
            }
        }
    }
}
