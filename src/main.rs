// soundbox-ii music playback controller *don't keep your sounds boxed up*
// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.
//
//! Binary crate for running the soundbox-ii logic

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
use tokio::sync::watch;

mod cli;

mod web;

mod args;

use task::{AsyncTasks, ShutdownReceiver};
use vlc_http::vlc_responses::UrlsFmt;
mod task;

#[tokio::main]
async fn main() {
    let args = args::parse_or_exit();

    eprint!("{}", cli::COMMAND_NAME);
    eprintln!("{}", shared::license::WELCOME);
    launch(args).await;
}

fn print_startup_info(args: &args::Config) {
    println!(
        "  - VLC-HTTP will connect to server: {}",
        args.vlc_http_config.0.authority_str()
    );
    if let Some(server_config) = args.server_config.as_ref() {
        println!(
            "  - Serving static assets from {:?}",
            server_config.static_assets
        );
        if server_config.watch_assets {
            println!("    - Watching for changes, will notify clients");
        }
        println!("  - Listening on: {}", server_config.bind_address);
    }
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

struct WebSourceChanged;
fn launch_hotwatch(
    server_config: &args::ServerConfig,
    reload_tx: watch::Sender<WebSourceChanged>,
) -> hotwatch::Hotwatch {
    let mut hotwatch = hotwatch::Hotwatch::new().expect("hotwatch failed to initialize");
    hotwatch
        .watch(server_config.static_assets.clone(), move |event| {
            use hotwatch::Event;
            match event {
                Event::NoticeWrite(_) | Event::NoticeRemove(_) => {
                    // ignore "Notice" events, files are not actively reading
                }
                _ => reload_tx.send_modify(|WebSourceChanged| {}),
            }
        })
        .expect("static assets folder not found");
    hotwatch
}

async fn launch(args: args::Config) {
    let (cli_shutdown_tx, shutdown_rx) = ShutdownReceiver::new();
    let (reload_tx, reload_rx) = watch::channel(WebSourceChanged);

    print_startup_info(&args);
    let is_interactive = args.is_interactive();

    let authorization = args.vlc_http_config.0;
    let (controller, channels) = vlc_http::Controller::new(authorization);
    let vlc_http::controller::ExternalChannels {
        action_tx,
        playback_status_rx,
        playlist_info_rx,
        cmd_playlist_tx,
    } = channels;

    let cli_handle = if is_interactive {
        let action_tx = action_tx.clone();
        let playback_status_rx = playback_status_rx.clone();
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

    let hotwatch_handle = args.server_config.as_ref().and_then(|server_config| {
        if server_config.watch_assets {
            Some(launch_hotwatch(server_config, reload_tx))
        } else {
            None
        }
    });

    // spawn server
    let warp_graceful_handle = args.server_config.map(|server_config| {
        const TASK_NAME: &str = "warp";
        let api = {
            let action_tx = action_tx.clone();
            let playback_status_rx = playback_status_rx.clone();
            web::filter(
                web::Config {
                    action_tx,
                    playback_status_rx,
                    // playlist_info_rx,
                    reload_rx,
                },
                server_config.static_assets,
            )
        };
        let shutdown_rx = shutdown_rx.clone();
        let (_addr, server) =
            warp::serve(api).bind_with_graceful_shutdown(server_config.bind_address, async move {
                shutdown_rx.wait_for_shutdown(TASK_NAME).await;
                println!("waiting for warp HTTP clients to disconnect..."); // TODO: add mechanism to ask WebSocket ClientHandlers to disconnect
            });
        tokio::task::spawn(async {
            server.await;
            println!("ended: {}", TASK_NAME);
        })
    });

    let mut tasks = AsyncTasks::new(shutdown_rx);

    // run controller
    tasks.spawn("vlc controller", controller.run());

    // // run Sequencer test add-in
    // tasks.spawn("sequencer", async move {
    //     test_sequencer_fn(cmd_playlist_tx).await
    // });

    // join all async tasks and thread(s)
    tasks.join_all().await.expect("tasks end with no panics");
    if let Some(warp_handle) = warp_graceful_handle {
        warp_handle.await.expect("warp ends with no panic");
    }
    if let Some(cli_handle) = cli_handle {
        cli_handle.join().expect("cli ends with no panic");
    }
    drop(hotwatch_handle); //explicit drop, **after** tasks shutdown

    // end of MAIN
    println!("[main exit]");
}

async fn test_sequencer_fn(
    cmd_playlist_tx: vlc_http::cmd_playlist_items::Sender,
) -> Result<shared::Never, Shutdown> {
    // TODO move logic to `sequencer`, with sequencer::Command passing channels
    // (e.g. to allow both `web` and `cli` to command and examine the sequencer)
    let vlc_http::cmd_playlist_items::Sender {
        urls_tx,
        mut remove_rx,
    } = cmd_playlist_tx;
    urls_tx.send_modify(|data| {
        data.items = (0..7)
            .map(|n| cli::parse_url(&format!("{n}AM.mp3")).expect("valid test url"))
            .collect();
    });
    while let Ok(()) = remove_rx.changed().await {
        let removed_url_str = remove_rx.borrow().clone();
        urls_tx.send_modify(|data| {
            match data.items.first() {
                Some(first) if first.to_string() == removed_url_str => {
                    data.items.remove(0);
                }
                Some(first_mismatched) => {
                    dbg!("mismatch!! ohno!", first_mismatched);
                }
                None => {
                    println!("all done");
                }
            }
            dbg!(UrlsFmt(&data.items[..]));
            dbg!(data.max_history_count);
        });
    }
    // TODO is this shutdown correct?
    Err(Shutdown)
}
