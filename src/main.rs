use tokio::sync::mpsc;

use cli::Prompt;
mod cli;

mod web {
    pub use filter::root as filter;
    mod filter {
        use http::uri::Uri;
        use std::path::PathBuf;
        use tokio::sync::mpsc;
        use vlc_http::Action;
        use warp::Filter;

        pub fn root(
            action_tx: mpsc::Sender<Action>,
            assets_dir: PathBuf,
        ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
            root_redirect()
                .or(warp::path("v1").and(api_v1::root()))
                .or(static_files(assets_dir))
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

    println!("  - Listening on: {}", args.bind_address);
    println!("  - Serving static assets from {:?}", args.static_assets);
    println!(
        "  - VLC-HTTP will connect to server: {}",
        args.vlc_http_credentials.authority_str()
    );

    let api = web::filter(action_tx.clone(), args.static_assets);

    if args.interactive {
        // spawn prompt
        std::thread::spawn(move || {
            Prompt::new(action_tx).run().unwrap();
        });
    }

    // spawn server
    let server = warp::serve(api).bind(args.bind_address);
    tokio::task::spawn(server);

    // run controller
    vlc_http::run(args.vlc_http_credentials, action_rx).await;
}
