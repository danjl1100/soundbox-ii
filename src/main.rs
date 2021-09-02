//! Binary crate for running the soundbox-ii logic

// TODO: only while building
#![allow(dead_code)]
// teach me
#![deny(clippy::pedantic)]
// no unsafe
#![forbid(unsafe_code)]
// no unwrap
#![deny(clippy::unwrap_used)]
// no panic
#![deny(clippy::panic)]
// docs!
#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

use shared::{Never, Shutdown};
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
            shutdown_rx: crate::ShutdownReceiver,
            reload_rx: watch::Receiver<u32>,
            assets_dir: PathBuf,
        ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
            root_redirect()
                .or(warp::path("v1").and(api_v1::root()))
                .or(static_files(assets_dir))
                .or(super::web_socket::filter(super::web_socket::Config {
                    action_tx,
                    playback_status_rx,
                    playlist_info_rx,
                    shutdown_rx,
                    reload_rx,
                }))
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
        use crate::ShutdownReceiver;
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
            pub reload_rx: watch::Receiver<u32>,
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
                // invalidate pending "changed" values
                {
                    self.config.playback_status_rx.borrow_and_update();
                    self.config.playlist_info_rx.borrow_and_update();
                }
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
                            let playback = (*self.config.playback_status_rx.borrow())
                                .as_ref()
                                .map(vlc_http::PlaybackStatus::clone_to_shared)
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
}

mod args;

#[tokio::main]
async fn main() {
    // bug in clippy and/or tokio proc macro
    //  ref:  https://github.com/rust-lang/rust-clippy/issues/7438
    #![allow(clippy::semicolon_if_nothing_returned)]

    let args = args::parse_or_exit();

    println!("\nHello, soundbox-ii!\n");
    launch(args).await;
}

use task::{AsyncTasks, ShutdownReceiver};
mod task {
    use shared::{Never, Shutdown};
    use tokio::sync::watch;

    /// Receiver for the [`Shutdown`] signal
    #[derive(Clone)]
    pub struct ShutdownReceiver(watch::Receiver<Option<Shutdown>>);
    impl ShutdownReceiver {
        /// Constructs a [`watch::Sender`] and [`ShutdownReceiver`] pair
        pub fn new() -> (watch::Sender<Option<Shutdown>>, Self) {
            let (tx, rx) = watch::channel(None);
            (tx, Self(rx))
        }
        /// Synchronous poll for Shutdown
        pub fn poll_shutdown(&self, task_name: &'static str) -> Option<Shutdown> {
            let value = *self.0.borrow();
            if let Some(Shutdown) = value {
                println!("{} received shutdown", task_name);
            }
            value
        }
        /// Asynchronous poll for Shutdown
        pub async fn is_shutdown(&mut self, task_name: &'static str) -> Option<Shutdown> {
            let rx = &mut self.0;
            let changed_result = rx.changed().await;
            if changed_result.is_err() {
                eprintln!(
                    "error waiting for {} shutdown signal, shutting down...",
                    task_name
                );
                Some(Shutdown)
            } else {
                let shutdown = *rx.borrow();
                if shutdown.is_some() {
                    println!("received shutdown: {}", task_name);
                }
                shutdown
            }
        }
        /// Asynchronous wait for Shutdown
        pub async fn wait_for_shutdown(mut self, task_name: &'static str) {
            while self.is_shutdown(task_name).await.is_none() {
                continue;
            }
        }
    }

    pub struct AsyncTasks {
        handles: Vec<tokio::task::JoinHandle<()>>,
        shutdown_rx: ShutdownReceiver,
    }
    impl AsyncTasks {
        /// Creates an empty instance, using the specified [`ShutdownReceiver`] to abort tasks
        pub fn new(shutdown_rx: ShutdownReceiver) -> Self {
            Self {
                handles: vec![],
                shutdown_rx,
            }
        }
        /// Spawns a new async task, to be cancelled when Shutdown is received
        pub fn spawn(
            &mut self,
            task_name: &'static str,
            task: impl std::future::Future<Output = Result<Never, Shutdown>> + Send + 'static,
        ) {
            let mut shutdown_rx = self.shutdown_rx.clone();
            let handle = tokio::task::spawn(async move {
                tokio::select! {
                    biased; // poll in-order (shutdown first)
                    Some(Shutdown) = shutdown_rx.is_shutdown(task_name) => {}
                    Err(Shutdown) = task => {}
                    else => {}
                };
                println!("ended: {}", task_name);
            });
            self.handles.push(handle);
        }
        /// Waits for all tasks to complete
        ///
        /// # Errors
        /// Returns an error if any task fails to join
        pub async fn join_all(self) -> Result<(), tokio::task::JoinError> {
            for task in self.handles {
                task.await?;
            }
            Ok(())
        }
    }
}

#[allow(clippy::too_many_lines)] //TODO: modularize this init script
async fn launch(args: args::Config) {
    let (action_tx, action_rx) = mpsc::channel(1);
    let (playback_status_tx, playback_status_rx) = watch::channel(None);
    let (playlist_info_tx, playlist_info_rx) = watch::channel(None);
    let (cli_shutdown_tx, shutdown_rx) = ShutdownReceiver::new();
    let (reload_tx, reload_rx) = watch::channel(0_u32);

    println!(
        "  - VLC-HTTP will connect to server: {}",
        args.vlc_http_credentials.authority_str()
    );
    println!("  - Serving static assets from {:?}", args.static_assets);
    if args.watch_assets {
        println!("    - Watching for changes, will notify clients");
    }
    println!("  - Listening on: {}", args.bind_address);
    println!();
    // ^^^ listen URL is last (for easy skimming)

    let cli_handle = if args.interactive {
        const TASK_NAME: &str = "cli";
        // spawn prompt
        let action_tx = action_tx.clone();
        let playback_status_rx = playback_status_rx.clone();
        let playlist_info_rx = playlist_info_rx.clone();
        let shutdown_rx = shutdown_rx.clone();
        let handle = std::thread::spawn(move || {
            cli::Config {
                action_tx,
                playback_status_rx,
                playlist_info_rx,
            }
            .build()
            .run_until(move || shutdown_rx.poll_shutdown(TASK_NAME))
            .expect("cli free from IO errors");
            let _ = cli_shutdown_tx.send(Some(Shutdown));
            println!("{} ended", TASK_NAME);
        });
        Some(handle)
    } else {
        None
    };

    let _hotwatch_handle = if args.watch_assets {
        let mut hotwatch = hotwatch::Hotwatch::new().expect("hotwatch failed to initialize");
        hotwatch
            .watch(args.static_assets.clone(), move |event| {
                use hotwatch::Event;
                match event {
                    Event::NoticeWrite(_) | Event::NoticeRemove(_) => {
                        // ignore "Notice" events, files are not actively reading
                        // println!("ignoring {:?}", event);
                    }
                    _ => {
                        // println!("changed! {:?}", event);
                        let prev_value = *reload_tx.borrow();
                        let next_value = prev_value.wrapping_add(1);
                        reload_tx
                            .send(next_value)
                            .expect("reload receiver is alive");
                    }
                }
            })
            .expect("static assets folder not found");
        Some(hotwatch)
    } else {
        None
    };

    // spawn server
    let warp_graceful_handle = {
        const TASK_NAME: &str = "warp";
        let api = web::filter(
            action_tx.clone(),
            playback_status_rx.clone(),
            playlist_info_rx,
            shutdown_rx.clone(),
            reload_rx,
            args.static_assets,
        );
        let shutdown_rx = shutdown_rx.clone();
        let (_addr, server) =
            warp::serve(api).bind_with_graceful_shutdown(args.bind_address, async move {
                shutdown_rx.wait_for_shutdown(TASK_NAME).await;
                println!("waiting for warp HTTP clients to disconnect..."); // TODO: add mechanism to ask WebSocket ClientHandlers to disconnect
            });
        tokio::task::spawn(async {
            server.await;
            println!("ended: {}", TASK_NAME);
        })
    };

    let mut tasks = AsyncTasks::new(shutdown_rx);

    // spawn PlaybackStatus requestor
    {
        async fn playback_status_requestor(
            action_tx: mpsc::Sender<vlc_http::Action>,
            playback_status_rx: watch::Receiver<Option<vlc_http::PlaybackStatus>>,
        ) -> Result<Never, Shutdown> {
            use tokio::time::Duration;
            use vlc_http::vlc_responses::PlaybackState;
            const DELAY_SEC_MIN: u64 = 1;
            const DELAY_SEC_SHORT: u64 = 20;
            const DELAY_SEC_LONG: u64 = 90;
            loop {
                let (cmd, result_rx) = vlc_http::Action::query_playback_status();
                action_tx.send(cmd).await.map_err(|err| {
                    eprintln!("error auto-requesting PlaylistStatus: {}", err);
                    Shutdown
                })?;
                print!("fetching PlaybackStatus...  ");
                {
                    use std::io::Write;
                    drop(std::io::stdout().lock().flush());
                }
                let sleep_duration = match result_rx.await {
                    Err(err) => {
                        eprintln!("vlc_http module did not respond :/  {}", err);
                        Err(Shutdown)
                    }
                    Ok(Err(err)) => {
                        eprintln!("error in result: {:?}", err);
                        Ok(Duration::from_secs(DELAY_SEC_LONG))
                    }
                    Ok(Ok(_)) => {
                        println!("fetch done");
                        let remaining = match &*playback_status_rx.borrow() {
                            Some(playback) if playback.state == PlaybackState::Playing => {
                                Some(playback.duration.saturating_sub(playback.time))
                            }
                            _ => None,
                        };
                        let delay = remaining.map_or(DELAY_SEC_SHORT, |remaining| {
                            remaining.min(DELAY_SEC_SHORT).max(DELAY_SEC_MIN)
                        });
                        Ok(Duration::from_secs(delay))
                    }
                };
                tokio::time::sleep(sleep_duration?).await;
            }
        }
        let action_tx = action_tx.clone();
        let playback_status_rx = playback_status_rx.clone();
        tasks.spawn("PlaybackStatus-requestor", async {
            playback_status_requestor(action_tx, playback_status_rx).await
        });
    }

    // run controller
    {
        tasks.spawn(
            "vlc controller",
            vlc_http::controller::Config {
                action_rx,
                playback_status_tx,
                playlist_info_tx,
                credentials: args.vlc_http_credentials,
            }
            .build()
            .run(),
        );
    }

    // join all async tasks and thread(s)
    tasks.join_all().await.expect("tasks end with no panics");
    warp_graceful_handle.await.expect("warp ends with no panic");
    if let Some(cli_handle) = cli_handle {
        cli_handle.join().expect("cli ends with no panic");
    }

    // end of MAIN
    println!("[main exit]");
}
