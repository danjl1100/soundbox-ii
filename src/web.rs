pub use filter::root as filter;
mod filter {
    use http::uri::Uri;
    use std::path::PathBuf;
    use warp::Filter;

    pub fn root(
        config: super::web_socket::Config,
        assets_dir: PathBuf,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        root_redirect()
            .or(warp::path("v1").and(api_v1::root()))
            .or(static_files(assets_dir))
            .or(super::web_socket::filter(config))
    }

    fn root_redirect() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        // NOTE: temporary, in case we change it later
        warp::path::end().map(|| warp::redirect::temporary(Uri::from_static("/app/")))
    }

    fn static_files(
        assets_dir: PathBuf,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::get()
            .and(warp::path("app"))
            .and(warp::fs::dir(assets_dir))
    }

    mod api_v1 {
        use warp::Filter;

        pub fn root(
        ) -> impl Filter<Extract = (String,) /*impl warp::Reply*/, Error = warp::Rejection> + Clone
        {
            warp::get().and(test_number_random())
        }
        fn test_number_random() -> impl Filter<Extract = (String,), Error = warp::Rejection> + Clone
        {
            use std::sync::atomic::{AtomicU32, Ordering};
            use std::sync::Arc;
            let atomic_num = Arc::new(AtomicU32::new(27));
            warp::path("number").map(move || {
                let value = atomic_num.fetch_add(1, Ordering::Relaxed);
                let title = if value % 3 == 0 {
                    "the BEST number"
                } else {
                    "an OKAY number"
                }
                .to_string();
                let number = shared::Number {
                    value,
                    title,
                    is_even: value % 2 == 0,
                };
                serde_json::to_string(&number).expect("Serializes Number without error")
            })
        }
    }
}

pub(crate) use web_socket::Config;
mod web_socket {
    use crate::{ReloadVersion, ShutdownReceiver};
    use futures::{SinkExt, StreamExt};
    use shared::{ClientRequest, ServerResponse};
    use tokio::sync::{mpsc, watch};
    use vlc_http::{Action, IntoAction, PlaybackStatus, PlaylistInfo};
    use warp::ws::Message;
    use warp::Filter;

    #[derive(Clone)]
    pub struct Config {
        pub action_tx: mpsc::Sender<Action>,
        pub playback_status_rx: watch::Receiver<Option<PlaybackStatus>>,
        pub playlist_info_rx: watch::Receiver<Option<PlaylistInfo>>, //TODO use this field, or remove it!
        pub shutdown_rx: ShutdownReceiver,
        pub reload_rx: watch::Receiver<ReloadVersion>,
    }
    pub fn filter(
        config: Config,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
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
            let reload_base_value = *self.config.reload_rx.borrow_and_update();
            self.send_response(ServerResponse::Heartbeat).await?;
            loop {
                let send_message = tokio::select! {
                    Ok(_) = self.config.reload_rx.changed() => {
                        if reload_base_value == *self.config.reload_rx.borrow() {
                            // borrowed value was updated to identical value...  LOGIC ERROR!
                            // however... silently proceed (non-critical ease-of-use feature)
                            None
                        } else {
                            Some(ServerResponse::ClientCodeChanged)
                        }
                    }
                    Some(body) = self.websocket.next() => {
                        let message = match body {
                            Ok(msg) => msg,
                            Err(e) => {
                                eprintln!("Error reading message on websocket: {}", e);
                                break;
                            }
                        };
                        // tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                        self.handle_message(message).await
                    }
                    Ok(_) = self.config.playback_status_rx.changed() => {
                        let now = chrono::Utc::now();
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
            dbg!(&msg);
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
                    dbg!(&command);
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
            let send_result = self.config.action_tx.send(action).await;
            match send_result {
                Ok(()) => {
                    let recv_result = result_rx.await;
                    dbg!(&recv_result);
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
