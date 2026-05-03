// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Library helper for faking a `vlc` HTTP server in end-to-end integration tests

#![expect(missing_docs, reason = "figuring out the right API")] // TODO remove

use eyre::Context as _;
use std::sync::Arc;

use crate::only_socket_addr::SocketServer;

type SpawnHandle = std::thread::JoinHandle<Result<(), std::io::Error>>;

/// Server mimicking the VLC HTTP interface
pub struct FakeVlc {
    server: Arc<SocketServer>,
    fake_password: String,
}
impl FakeVlc {
    /// Binds the HTTP server to an OS-provided port at localhost, for use in `spawn`
    ///
    /// # Errors
    /// Returns an error if the server bind fails
    pub fn new() -> eyre::Result<(Self, SpawnHandle)> {
        Self::new_bind_to("127.0.0.1:0")
    }
    /// Binds the HTTP server, for use in `spawn`
    ///
    /// # Errors
    /// Returns an error if the server bind fails
    pub fn new_bind_to(
        bind_address: impl std::net::ToSocketAddrs,
    ) -> eyre::Result<(Self, SpawnHandle)> {
        let server = SocketServer::http(bind_address)
            .map_err(|e| eyre::eyre!(e))
            .context("failed to bind FakeVlc server")?;
        let server = Arc::new(server);

        let fake_password: [u8; 8] = rand::random();
        let fake_password = fake_password.into_iter().fold(String::new(), |mut acc, v| {
            use std::fmt::Write as _;
            write!(&mut acc, "{v:02x}").expect("infallible");
            acc
        });

        let this = Self {
            server,
            fake_password,
        };

        let handle = this.spawn();

        Ok((this, handle))
    }
    /// Returns the auth info required to connect to the HTTP server
    ///
    /// NOTE: Clones both the password and IP address strings
    #[must_use]
    pub fn get_auth_cloned(&self) -> vlc_http_auth::AuthInput {
        let addr = self.server.get_server_addr();
        let vlc_host = vlc_http_auth::Host(addr.ip().to_string());
        let vlc_port = vlc_http_auth::Port(addr.port());

        let vlc_password = vlc_http_auth::Password(self.fake_password.clone());

        vlc_http_auth::AuthInput {
            vlc_password,
            vlc_host,
            vlc_port,
        }
    }
    /// Borrows `self` to spawn an HTTP receive thread in a scope
    #[must_use]
    fn spawn(&self) -> SpawnHandle {
        let Self { server, .. } = self;
        // weak reference, to end the loop after receiving a wakeup
        let server = Arc::downgrade(server);
        std::thread::spawn(move || {
            loop {
                let Some(server) = server.upgrade() else {
                    // server dropped, after a request was received
                    break Ok(());
                };
                let request = match server.inner().recv() {
                    Ok(req) => req,
                    Err(e)
                        if e.kind() == std::io::ErrorKind::Other
                            && e.to_string() == "thread unblocked" =>
                    {
                        // server dropped, then unblocked with no request
                        break Ok(());
                    }
                    Err(e) => {
                        println!("FakeVlc error: {e}");
                        break Err::<(), _>(e);
                    }
                };

                // TODO function, organize for logical state read/respond
                dbg!(request.url());
                let response =
                    tiny_http::Response::from_string("Not really sure").with_status_code(501);
                let result = request.respond(response);
                if let Err(e) = result {
                    eprintln!("FakeVlc response error: {:?}", eyre::eyre!(e));
                }
            }
        })
    }
    #[must_use]
    pub fn get_playlist(&self) -> Vec<()> {
        todo!()
    }
}
impl Drop for FakeVlc {
    fn drop(&mut self) {
        let Self {
            server,
            fake_password: _,
        } = self;
        while Arc::weak_count(server) > 0 {
            server.inner().unblock();
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
}

mod only_socket_addr {
    //! Invariants:
    //! - [`SocketServer`] only binds to socket addresses
    //!
    //! (e.g. stricter than `tiny_http::Server` which can be made from
    //! listener to wrap a Unix domain socket)

    use std::net::SocketAddr;

    /// Newtype wrapper around [`tiny_http::Server`] that has a socket address
    /// (not Unix domain socket)
    pub struct SocketServer(tiny_http::Server);
    impl SocketServer {
        pub fn http(
            bind_address: impl std::net::ToSocketAddrs,
        ) -> Result<SocketServer, Box<dyn std::error::Error + Send + Sync>> {
            tiny_http::Server::http(bind_address).map(Self)
        }
        pub fn inner(&self) -> &tiny_http::Server {
            let Self(server) = self;
            server
        }
        #[must_use]
        pub fn get_server_addr(&self) -> SocketAddr {
            self.inner()
                .server_addr()
                .to_ip()
                .expect("bound to an IP address")
        }
    }
}
