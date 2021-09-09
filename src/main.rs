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

use shared::Shutdown;
use tokio::sync::{mpsc, watch};

mod cli;

mod web;

mod args;

use task::{AsyncTasks, ShutdownReceiver};
mod task;

#[tokio::main]
async fn main() {
    // bug in clippy and/or tokio proc macro
    //  ref:  https://github.com/rust-lang/rust-clippy/issues/7438
    #![allow(clippy::semicolon_if_nothing_returned)]

    let args = args::parse_or_exit();

    println!("\nHello, soundbox-ii!\n");
    launch(args).await;
}

/// Version of a file to reload
#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
pub struct ReloadVersion(u32);
impl std::ops::Deref for ReloadVersion {
    type Target = u32;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

fn print_startup_info(args: &args::Config) {
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
}

fn launch_cli(
    config: cli::Config,
    shutdown_rx: ShutdownReceiver,
    cli_shutdown_tx: watch::Sender<Option<Shutdown>>,
) -> std::thread::JoinHandle<()> {
    const TASK_NAME: &str = "cli";
    // spawn prompt
    std::thread::spawn(move || {
        config
            .build()
            .run_until(move || shutdown_rx.poll_shutdown(TASK_NAME))
            .expect("cli free from IO errors");
        let _ = cli_shutdown_tx.send(Some(Shutdown));
        println!("{} ended", TASK_NAME);
    })
}

fn launch_hotwatch(
    args: &args::Config,
    reload_tx: watch::Sender<ReloadVersion>,
) -> hotwatch::Hotwatch {
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
                        .send(ReloadVersion(next_value))
                        .expect("reload receiver is alive");
                }
            }
        })
        .expect("static assets folder not found");
    hotwatch
}

//TODO - remove, superceded by `vlc_http::controller::rules`
// async fn playback_status_requestor(
//     action_tx: mpsc::Sender<vlc_http::Action>,
//     playback_status_rx: watch::Receiver<Option<vlc_http::PlaybackStatus>>,
// ) -> Result<Never, Shutdown> {
//     use tokio::time::Duration;
//     use vlc_http::vlc_responses::PlaybackState;
//     const DELAY_SEC_MIN: u64 = 1;
//     const DELAY_SEC_SHORT: u64 = 20;
//     const DELAY_SEC_LONG: u64 = 90;
//     loop {
//         let (cmd, result_rx) = vlc_http::Action::query_playback_status();
//         action_tx.send(cmd).await.map_err(|err| {
//             eprintln!("error auto-requesting PlaylistStatus: {}", err);
//             Shutdown
//         })?;
//         print!("fetching PlaybackStatus...  ");
//         {
//             use std::io::Write;
//             drop(std::io::stdout().lock().flush());
//         }
//         let sleep_duration = match result_rx.await {
//             Err(err) => {
//                 eprintln!("vlc_http module did not respond :/  {}", err);
//                 Err(Shutdown)
//             }
//             Ok(Err(err)) => {
//                 eprintln!("error in result: {:?}", err);
//                 Ok(Duration::from_secs(DELAY_SEC_LONG))
//             }
//             Ok(Ok(_)) => {
//                 println!("fetch done");
//                 let remaining = match &*playback_status_rx.borrow() {
//                     Some(playback) if playback.state == PlaybackState::Playing => {
//                         Some(playback.duration.saturating_sub(playback.time))
//                     }
//                     _ => None,
//                 };
//                 let delay = remaining.map_or(DELAY_SEC_SHORT, |remaining| {
//                     remaining.min(DELAY_SEC_SHORT).max(DELAY_SEC_MIN)
//                 });
//                 Ok(Duration::from_secs(delay))
//             }
//         };
//         tokio::time::sleep(sleep_duration?).await;
//     }
// }

async fn launch(args: args::Config) {
    let (action_tx, action_rx) = mpsc::channel(1);
    let (playback_status_tx, playback_status_rx) = watch::channel(None);
    let (playlist_info_tx, playlist_info_rx) = watch::channel(None);
    let (cli_shutdown_tx, shutdown_rx) = ShutdownReceiver::new();
    let (reload_tx, reload_rx) = watch::channel(ReloadVersion::default());

    print_startup_info(&args);

    let cli_handle = if args.interactive {
        let action_tx = action_tx.clone();
        let playback_status_rx = playback_status_rx.clone();
        let playlist_info_rx = playlist_info_rx.clone();
        let shutdown_rx = shutdown_rx.clone();
        Some(launch_cli(
            cli::Config {
                action_tx,
                playback_status_rx,
                playlist_info_rx,
            },
            shutdown_rx,
            cli_shutdown_tx,
        ))
    } else {
        None
    };

    let hotwatch_handle = if args.watch_assets {
        Some(launch_hotwatch(&args, reload_tx))
    } else {
        None
    };

    // spawn server
    let warp_graceful_handle = {
        const TASK_NAME: &str = "warp";
        let api = {
            let action_tx = action_tx.clone();
            let playback_status_rx = playback_status_rx.clone();
            let shutdown_rx = shutdown_rx.clone();
            web::filter(
                web::Config {
                    action_tx,
                    playback_status_rx,
                    playlist_info_rx,
                    shutdown_rx,
                    reload_rx,
                },
                args.static_assets,
            )
        };
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

    //TODO - remove, superceded by `vlc_http::controller::rules`
    // // spawn PlaybackStatus requestor
    // tasks.spawn("PlaybackStatus-requestor", async {
    //     playback_status_requestor(action_tx, playback_status_rx).await
    // });

    // run controller
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

    // join all async tasks and thread(s)
    tasks.join_all().await.expect("tasks end with no panics");
    warp_graceful_handle.await.expect("warp ends with no panic");
    if let Some(cli_handle) = cli_handle {
        cli_handle.join().expect("cli ends with no panic");
    }
    drop(hotwatch_handle); //explicit drop, **after** tasks shutdown

    // end of MAIN
    println!("[main exit]");
}
