// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Library helper for faking a `vlc` HTTP server in end-to-end integration tests

#![expect(missing_docs, reason = "figuring out the right API")] // TODO remove

use eyre::Context as _;
use std::{
    ops::ControlFlow,
    sync::{Arc, Mutex},
};
use vlc_http_test::Model;

use crate::only_socket_addr::SocketServer;

pub use self::arbtest::{ArbTestWithFakeVlc, arbtest_with_fake_vlc};

mod arbtest;

type SpawnHandle = std::thread::JoinHandle<Result<(), std::io::Error>>;

/// Server mimicking the VLC HTTP interface
pub struct FakeVlc {
    inner_shared: Arc<InnerShared>,
}
pub struct InnerShared {
    server: SocketServer,
    fake_password: String,
    inner_mut: Mutex<InnerMut>,
}
pub struct InnerMut {
    model: Model,
}
impl FakeVlc {
    /// Calls the specified function with an instance and configured
    /// [`vlc_http_ureq::HttpRunner`]
    ///
    /// # Errors
    /// Returns an error if the server bind fails
    ///
    /// # Panics
    /// Panics if shutting down the spawned thread handle fails due to panic
    /// elsewhere in the program
    pub fn with_new<T>(
        test_fn: impl FnOnce(&FakeVlc, &mut vlc_http_ureq::HttpRunner) -> eyre::Result<T>,
    ) -> eyre::Result<T> {
        let (vlc, thread_handle) = FakeVlc::new()?;

        let auth = vlc_http_auth::Auth::new(vlc.get_auth_cloned())?;
        let mut endpoint_caller = vlc_http_ureq::HttpRunner::new(auth);

        let result = test_fn(&vlc, &mut endpoint_caller)?;

        drop(vlc);
        thread_handle.join().expect("VLC thread panic")?;

        Ok(result)
    }
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

        let fake_password: [u8; 8] = rand::random();
        let fake_password = fake_password.into_iter().fold(String::new(), |mut acc, v| {
            use std::fmt::Write as _;
            write!(&mut acc, "{v:02x}").expect("infallible");
            acc
        });

        let inner_mut = InnerMut {
            model: Model::default(),
        };
        let inner_mut = Mutex::new(inner_mut);

        let inner_shared = InnerShared {
            server,
            fake_password,
            inner_mut,
        };
        let inner_shared = Arc::new(inner_shared);

        let this = Self { inner_shared };

        let handle = this.spawn();

        Ok((this, handle))
    }
    /// Returns the auth info required to connect to the HTTP server
    ///
    /// NOTE: Clones both the password and IP address strings
    #[must_use]
    pub fn get_auth_cloned(&self) -> vlc_http_auth::AuthInput {
        let addr = self.inner_shared.server.get_server_addr();
        let vlc_host = vlc_http_auth::Host(addr.ip().to_string());
        let vlc_port = vlc_http_auth::Port(addr.port());

        let vlc_password = vlc_http_auth::Password(self.inner_shared.fake_password.clone());

        vlc_http_auth::AuthInput {
            vlc_password,
            vlc_host,
            vlc_port,
        }
    }
    /// Borrows `self` to spawn an HTTP receive thread in a scope
    #[must_use]
    fn spawn(&self) -> SpawnHandle {
        let Self { inner_shared } = self;

        // weak reference, to end the loop after receiving a wakeup
        let inner_shared = Arc::downgrade(inner_shared);
        std::thread::spawn(move || {
            loop {
                let Some(inner_shared) = inner_shared.upgrade() else {
                    // server dropped, after a request was received
                    break Ok(());
                };

                match inner_shared.recv_and_run_request() {
                    ControlFlow::Break(result) => {
                        if let Err(e) = &result {
                            println!("FakeVlc error: {e}");
                        }
                        break result;
                    }
                    ControlFlow::Continue(()) => {}
                }
            }
        })
    }
    /// Clones the playlist items currently present in the model
    ///
    /// # Panics
    ///
    /// Panics if the inner mutex is poisoned (another thread holding the mutex,
    /// e.g. spawned thread panicked)
    #[must_use]
    pub fn get_playlist_cloned(&self) -> Vec<vlc_http_test::model::Item> {
        let inner_mut = self.inner_shared.inner_mut.lock().expect("no poison");
        inner_mut.model.get_items().to_vec()
    }
}
impl Drop for FakeVlc {
    fn drop(&mut self) {
        let Self { inner_shared, .. } = self;
        while Arc::weak_count(inner_shared) > 0 {
            inner_shared.server.inner().unblock();
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
}

impl InnerShared {
    fn recv_and_run_request(&self) -> ControlFlow<std::io::Result<()>> {
        let Self {
            server,
            fake_password: _, // TODO
            inner_mut,
        } = self;

        let request = match server.inner().recv() {
            Ok(req) => req,
            Err(e) => {
                let result = if e.kind() == std::io::ErrorKind::Other
                    && e.to_string() == "thread unblocked"
                {
                    // server dropped, then unblocked with no request
                    ControlFlow::Break(Ok(()))
                } else {
                    ControlFlow::Break(Err(e))
                };
                return result;
            }
        };

        let response_parts = match InnerMut::model_request(inner_mut, &request) {
            Ok(vlc_http_test::model::ModelResponse::Json(response)) => (response, None),
            Ok(vlc_http_test::model::ModelResponse::Art) => {
                ("request for Art".to_string(), Some(400))
            }
            Err(error) => (error.to_string(), Some(400)),
        };

        let response = {
            let (msg, code) = response_parts;
            let response = tiny_http::Response::from_string(msg);
            if let Some(code) = code {
                response.with_status_code(code)
            } else {
                response
            }
        };
        let result = request.respond(response);
        if let Err(e) = result {
            eprintln!("FakeVlc response error: {:?}", eyre::eyre!(e));
        }

        ControlFlow::Continue(())
    }
}
impl InnerMut {
    fn model_request(
        mutex: &Mutex<Self>,
        request: &tiny_http::Request,
    ) -> Result<vlc_http_test::model::ModelResponse, vlc_http_test::model::RequestError> {
        let mut inner_mut = mutex.lock().expect("no mutex poison");
        let Self { model } = &mut *inner_mut;
        model.request(request.url())
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
