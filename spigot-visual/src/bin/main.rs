// Copyright (C) 2021-2025  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Runner for the library [`spigot_visual`]

use tiny_http::{Header, Response};

use clap::Parser as _;
use spigot_visual::static_file;
use std::net::SocketAddr;

#[derive(clap::Parser)]
struct Config {
    #[clap(long, default_value = "127.0.0.1:8080")]
    bind_address: SocketAddr,

    #[clap(long)]
    dev_path_prefix: Option<String>,
}

fn main() -> eyre::Result<()> {
    let config = Config::parse();

    let server = match tiny_http::Server::http(config.bind_address) {
        Ok(server) => server,
        Err(e) => eyre::bail!(e),
    };

    eprintln!("Listening on {}...", config.bind_address);

    loop {
        let request = match server.recv() {
            Ok(request) => request,
            Err(e) => {
                eyre::bail!(e);
            }
        };

        match handle_request(request, &config) {
            Ok(()) => {}
            Err(e) => {
                let e = eyre::eyre!(e);
                eprintln!("failed to handle request: {e:?}");
            }
        }
    }
}

fn handle_request(request: tiny_http::Request, config: &Config) -> eyre::Result<()> {
    const CODE_301_MOVED: u32 = 301;
    const CODE_404_NOT_FOUND: u32 = 404;
    const INDEX: &str = "/index.html";

    let url = request.url();
    match url {
        s if s == INDEX => {
            static_file!("index.html").reply_html(request, config.dev_path_prefix.as_deref())
        }
        "/" => {
            let redirect = Response::empty(CODE_301_MOVED)
                .with_header(Header::from_bytes("Location", INDEX).expect("valid header"));
            request.respond(redirect)?;
            Ok(())
        }
        _ => {
            request.respond(Response::empty(CODE_404_NOT_FOUND))?;
            Ok(())
        }
    }
}
