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
            root_redirect().or(static_files(assets_dir))
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
    }
}

mod args {
    use std::convert::TryFrom;
    use std::net::SocketAddr;
    use std::path::PathBuf;
    use std::str::FromStr;

    pub struct Config {
        pub interactive: bool,
        pub bind_address: SocketAddr,
        pub vlc_http_credentials: vlc_http::Credentials,
        pub static_assets: PathBuf,
    }

    static INTERACTIVE: &str = "interactive";
    static BIND_ADDRESS: &str = "bind-address";
    static VLC_HOST: &str = "vlc-host";
    static VLC_PORT: &str = "vlc-port";
    static VLC_PASSWORD: &str = "vlc-password";
    static STATIC_ASSETS: &str = "static-assets";

    pub fn parse_or_exit() -> Config {
        use clap::{
            app_from_crate, crate_authors, crate_description, crate_name, crate_version, Arg,
        };
        let default_bind_address = SocketAddr::from(([127, 0, 0, 1], 3030)).to_string();
        let matches = app_from_crate!()
            .arg(
                Arg::with_name(INTERACTIVE)
                    .short("i")
                    .long(INTERACTIVE)
                    .help("Activates the command-line interface"),
            )
            .arg(
                Arg::with_name(BIND_ADDRESS)
                    .short("b")
                    .long(BIND_ADDRESS)
                    .default_value(&default_bind_address)
                    .help("Address and port to bind the HTTP server"),
            )
            .arg(
                Arg::with_name(VLC_HOST)
                    .long(VLC_HOST)
                    .takes_value(true)
                    .help("Address of VLC-HTTP server (overrides environment variable)"),
            )
            .arg(
                Arg::with_name(VLC_PORT)
                    .long(VLC_PORT)
                    .takes_value(true)
                    .help("Port of VLC-HTTP server (overrides environment variable)"),
            )
            .arg(
                Arg::with_name(VLC_PASSWORD)
                    .long(VLC_PASSWORD)
                    .takes_value(true)
                    .help("Password of VLC-HTTP server (overrides environment variable)"),
            )
            .arg(
                Arg::with_name(STATIC_ASSETS)
                    .long(STATIC_ASSETS)
                    .short("s")
                    .default_value("dist/")
                    .help("static asserts folder path (created by frontend)"),
            )
            .get_matches();

        match build_config(&matches) {
            Ok(config) => config,
            Err(message) => {
                eprintln!("{}", matches.usage());
                eprintln!();
                eprintln!("ERROR: {}", message);
                std::process::exit(1)
            }
        }
    }

    fn build_config(matches: &clap::ArgMatches<'_>) -> Result<Config, String> {
        let bind_address = matches
            .value_of(BIND_ADDRESS)
            .ok_or_else(|| "missing bind address".to_string())
            .and_then(|s| {
                SocketAddr::from_str(s)
                    .map_err(|err| format!("{} ({} argument \"{}\")", err, BIND_ADDRESS, s))
            })?;
        let static_assets = matches
            .value_of(STATIC_ASSETS)
            .ok_or_else(|| "missing static-assets folder".to_string())
            .and_then(|s| match PathBuf::from_str(s) {
                Err(err) => Err(format!("{} ({} argument \"{}\")", err, STATIC_ASSETS, s)),
                Ok(path) => match (path.exists(), path.is_dir()) {
                    (false, _) => Err(format!("static-assets path \"{}\" does not exist", s)),
                    (_, false) => Err(format!("static-assets path \"{}\" is not a folder", s)),
                    (true, true) => Ok(path),
                },
            })?;
        let vlc_http_credentials = build_vlc_credentials(matches)?;
        //
        Ok(Config {
            interactive: matches.is_present(INTERACTIVE),
            bind_address,
            vlc_http_credentials,
            static_assets,
        })
    }
    fn build_vlc_credentials(
        matches: &clap::ArgMatches<'_>,
    ) -> Result<vlc_http::Credentials, String> {
        use vlc_http::auth::{Config, Credentials, PartialConfig};
        const NOTE_CMD_HELP: &str =
            "NOTE: View command-line help (-h) for alternate methods of specifying VLC-HTTP parameters.";
        //
        let format_err_port = |(port_str, err)| format!("invalid port \"{}\" ({})", port_str, err);
        let format_err_partial =
            |partial| format!("incomplete VLC-HTTP {}\n{}", partial, NOTE_CMD_HELP);
        let format_err_uri =
            |(uri, uri_err)| format!("invalid VLC-HTTP host/port ({} \"{}\")", uri_err, uri);
        let unwrap_val = |key| matches.value_of(key).map(String::from).ok_or(());
        //
        let host = unwrap_val(VLC_HOST);
        let port = unwrap_val(VLC_PORT);
        let password = unwrap_val(VLC_PASSWORD);
        let arg_config = PartialConfig {
            host,
            port,
            password,
        };
        let merge_with_env = |arg_config| {
            let env_config = PartialConfig::from_env();
            Config::try_from_partial(env_config.override_with(arg_config))
        };
        let config = Config::try_from_partial(arg_config)
            .or_else(merge_with_env)
            .map(|result| result.map_err(format_err_port))
            .map_err(format_err_partial)??;
        Credentials::try_from(config).map_err(format_err_uri)
    }
}

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
