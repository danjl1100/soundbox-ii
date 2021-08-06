use std::convert::TryInto;
use tokio::sync::mpsc;

use cli::Prompt;
mod cli;

mod web {
    pub use filter::root as filter;
    mod filter {
        use http::uri::Uri;
        use tokio::sync::mpsc;
        use vlc_http::Action;
        use warp::Filter;

        pub fn root(
            action_tx: mpsc::Sender<Action>,
        ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
            root_redirect().or(static_files())
        }

        fn root_redirect(
        ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
            // NOTE: temporary, in case we change it later
            warp::path::end().map(|| warp::redirect::temporary(Uri::from_static("/app/")))
        }

        fn static_files() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone
        {
            warp::get()
                .and(warp::path("app"))
                .and(warp::fs::dir("dist/"))
        }
    }
}

#[tokio::main]
async fn main() {
    println!("\nHello, soundbox-ii!\n");

    let config = vlc_http::Config::try_from_env().expect("ENV vars set");
    let credentials = config.try_into().expect("valid host");
    println!("Will connect to: {:?}", credentials);

    let (action_tx, action_rx) = mpsc::channel(1);

    let api = web::filter(action_tx.clone());

    // spawn prompt
    std::thread::spawn(move || {
        Prompt::new(action_tx).run().unwrap();
    });

    // spawn server
    let server = warp::serve(api).bind(([127, 0, 0, 1], 3030));
    tokio::task::spawn(server);

    // run controller
    vlc_http::run(credentials, action_rx).await;
}
