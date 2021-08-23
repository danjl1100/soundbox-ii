use tokio::sync::{mpsc, watch};

mod cli;

mod web {
    pub use filter::root as filter;
    mod filter {
        use http::uri::Uri;
        use std::path::PathBuf;
        use tokio::sync::{mpsc, watch};
        use vlc_http::{Action, PlaybackStatus, PlaylistInfo};
        use warp::Filter;

        pub fn root(
            action_tx: mpsc::Sender<Action>,
            playback_status_rx: watch::Receiver<Option<PlaybackStatus>>,
            playlist_info_rx: watch::Receiver<Option<PlaylistInfo>>,
            assets_dir: PathBuf,
        ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
            root_redirect()
                .or(warp::path("v1").and(api_v1::root()))
                .or(static_files(assets_dir))
                .or(super::web_socket::filter(action_tx))
        }

        fn root_redirect(
        ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
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
            fn test_number_random(
            ) -> impl Filter<Extract = (String,), Error = warp::Rejection> + Clone {
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

    mod web_socket {
        use futures::{SinkExt, StreamExt};
        use shared::{ClientRequest, ServerResponse};
        use tokio::sync::mpsc;
        use vlc_http::{Action, IntoAction};
        use warp::ws::Message;
        use warp::Filter;

        pub fn filter(
            action_tx: mpsc::Sender<Action>,
        ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
            warp::path("ws")
                .and(warp::ws())
                .map(move |ws: warp::ws::Ws| {
                    let action_tx = action_tx.clone();
                    ws.on_upgrade(|websocket| {
                        ClientHandler {
                            websocket,
                            action_tx,
                        }
                        .run()
                    })
                })
        }
        struct ClientHandler {
            websocket: warp::ws::WebSocket,
            action_tx: mpsc::Sender<Action>,
        }
        impl ClientHandler {
            async fn run(mut self) {
                while let Some(body) = self.websocket.next().await {
                    let message = match body {
                        Ok(msg) => msg,
                        Err(e) => {
                            eprintln!("Error reading message on websocket: {}", e);
                            break;
                        }
                    };

                    self.handle_message(message).await;
                }
            }
            async fn handle_message(&mut self, message: Message) {
                // Skip any non-Text messages...
                let msg = if let Ok(s) = message.to_str() {
                    s
                } else {
                    println!("ping-pong");
                    return;
                };
                dbg!(&msg);
                let response = match serde_json::from_str(msg) {
                    Ok(request) => self.process_request(request).await,
                    Err(err) => {
                        dbg!(&err);
                        err.into()
                    }
                };

                // tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

                let response_str = serde_json::to_string(&response).unwrap();

                self.websocket
                    .send(Message::text(response_str))
                    .await
                    .unwrap();
            }
            async fn process_request(&mut self, request: ClientRequest) -> ServerResponse {
                match request {
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
                let send_result = self.action_tx.send(action).await;
                match send_result {
                    Ok(()) => {
                        let recv_result = result_rx.await;
                        dbg!(&recv_result);
                        match recv_result {
                            Ok(result) => ServerResponse::from_result(result),
                            Err(recv_err) => recv_err.into(),
                        }
                    }
                    Err(send_error) => {
                        dbg!(&send_error);
                        send_error.into()
                    }
                }
            }
        }
    }
}

mod args;

#[tokio::main]
async fn main() {
    let args = args::parse_or_exit();

    println!("\nHello, soundbox-ii!\n");
    launch(args).await;
}

async fn launch(args: args::Config) {
    let (action_tx, action_rx) = mpsc::channel(1);
    let (playback_status_tx, playback_status_rx) = watch::channel(Default::default());
    let (playlist_info_tx, playlist_info_rx) = watch::channel(Default::default());

    println!(
        "  - VLC-HTTP will connect to server: {}",
        args.vlc_http_credentials.authority_str()
    );
    println!("  - Serving static assets from {:?}", args.static_assets);
    println!("  - Listening on: {}", args.bind_address);
    // ^^^ listen URL is last (for easy skimming)

    if args.interactive {
        // spawn prompt
        let action_tx = action_tx.clone();
        let playback_status_rx = playback_status_rx.clone();
        let playlist_info_rx = playlist_info_rx.clone();
        std::thread::spawn(move || {
            cli::Config {
                action_tx,
                playback_status_rx,
                playlist_info_rx,
            }
            .build()
            .run()
            .unwrap();
        });
    }

    let api = web::filter(
        action_tx,
        playback_status_rx,
        playlist_info_rx,
        args.static_assets,
    );

    // spawn server
    let server = warp::serve(api).bind(args.bind_address);
    tokio::task::spawn(server);

    // run controller
    let vlc_controller = vlc_http::Controller {
        action_rx,
        playback_status_tx,
        playlist_info_tx,
    };
    vlc_controller.run(args.vlc_http_credentials).await;
}
