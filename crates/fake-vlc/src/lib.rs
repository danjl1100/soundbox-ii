// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Library helper for faking a `vlc` HTTP server in end-to-end integration tests

use eyre::Context as _;
use std::{ops::ControlFlow, sync::Arc};

use crate::{auth_result::AuthError, only_socket_addr::SocketServer};

pub use self::arbtest::{ArbTestWithFakeVlc, arbtest_with_fake_vlc};
use self::inner_mut::InnerMut;

mod arbtest;

type SpawnHandle = std::thread::JoinHandle<Result<(), std::io::Error>>;

/// Server mimicking the VLC HTTP interface
pub struct FakeVlc {
    inner_shared: Arc<InnerShared>,
}
struct InnerShared {
    server: SocketServer,
    fake_password: String,
    /// "Basic [base64]" version of `fake_password`
    fake_password_bearer: String,
    inner_mut: InnerMut,
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

        let fake_password_bearer = {
            use base64::{Engine as _, prelude::BASE64_STANDARD};
            let user_pass = format!(":{fake_password}");
            format!("Basic {}", BASE64_STANDARD.encode(&user_pass))
        };

        let inner_shared = InnerShared {
            server,
            fake_password,
            fake_password_bearer,
            inner_mut: InnerMut::new(),
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
    /// Writes the VLC config file content in the specified folder and filename,
    /// returning the complete path
    ///
    /// # Errors
    /// Returns an error if writing the file fails
    pub fn create_config_file(
        &self,
        dir: &std::path::Path,
        file_name: &str,
    ) -> eyre::Result<std::path::PathBuf> {
        let auth = self.get_auth_cloned();
        let file_content =
            toml::to_string_pretty(&auth).context("failed to serialize fake_vlc::AuthInput")?;
        create_config_file(dir, file_name, &file_content)
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
        self.inner_shared
            .inner_mut
            .lock_with_model_ref(|model| model.get_items().to_vec())
    }
    /// Serializes the model state into a JSON string
    ///
    /// # Panics
    ///
    /// Panics if the inner mutex is poisoned (another thread holding the mutex,
    /// e.g. spawned thread panicked)
    #[must_use]
    pub fn get_json_str(&self) -> String {
        let json_result = self.inner_shared.inner_mut.lock_with_model_ref(|model| {
            let json = model.as_json();
            serde_json::to_string_pretty(&json)
        });
        json_result.expect("serialize")
    }
    /// Waits for a "next" item to be available (per
    /// [`vlc_http_test::Model::get_available_next_track`]) then returns
    /// `Some(())` after advancing the VLC model to be playing that track.
    ///
    /// Returns `None` if the timeout expires with no change in available items
    #[expect(clippy::must_use_candidate, reason = "return diagnostic is optional")]
    pub fn wait_for_play_next(&self, wait_timeout: std::time::Duration) -> Option<()> {
        self.inner_shared.inner_mut.wait_for_play_next(wait_timeout)
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

/// Writes the config file content in the specified folder and filename, returning the complete path
///
/// # Errors
/// Returns an error if writing the file fails
pub fn create_config_file(
    dir: &std::path::Path,
    file_name: &str,
    file_content: &str,
) -> eyre::Result<std::path::PathBuf> {
    let mut p = dir.to_path_buf();
    p.push(file_name);
    std::fs::write(&p, file_content)
        .with_context(|| format!("failed to create {file_name} at {}", p.display()))?;

    Ok(p)
}

impl InnerShared {
    fn recv_and_run_request(&self) -> ControlFlow<std::io::Result<()>> {
        let Self {
            server,
            fake_password: _, // bearer only, raw not used
            fake_password_bearer,
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

        let response_parts = match AuthError::from_request(&request, fake_password_bearer) {
            Ok(()) => match InnerMut::lock_model_request(inner_mut, &request) {
                Ok(vlc_http_test::model::ModelResponse::Json(response)) => (response, None),
                Ok(vlc_http_test::model::ModelResponse::Art) => {
                    ("request for Art".to_string(), Some(400))
                }
                Err(error) => (error.to_string(), Some(400)),
            },
            Err(auth_err) => {
                eprintln!("{auth_err}");
                (String::new(), Some(auth_err.http_code()))
            }
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

mod inner_mut {
    //! Invariants:
    //! - Notifies [`InnerMut::current_playing`] when the Model's current playing item changes

    use std::sync::{Condvar, Mutex};

    use vlc_http_test::{
        Model,
        model::{ModelResponse, PlayState, RequestError},
    };

    pub struct InnerMut {
        model: Mutex<Model>,
        current_playing: Condvar,
    }
    impl InnerMut {
        pub fn new() -> Self {
            Self {
                model: Mutex::new(Model::default()),
                current_playing: Condvar::new(),
            }
        }
        pub fn lock_model_request(
            &self,
            request: &tiny_http::Request,
        ) -> Result<ModelResponse, RequestError> {
            let Self {
                model,
                current_playing,
            } = self;
            let (result, changed) = {
                let mut model = model.lock().expect("no mutex poison");
                let prev_current_playing = model.get_current_playing();
                let prev_items_len = model.get_items().len();

                tracing::debug!(url = request.url(), "model request");
                let result = model.request(request.url());

                let changed = model.get_current_playing() != prev_current_playing
                    || model.get_items().len() != prev_items_len;
                (result, changed)
            };

            if changed {
                current_playing.notify_all();
            }
            result
        }
        pub fn wait_for_play_next(&self, wait_timeout: std::time::Duration) -> Option<()> {
            let Self {
                model,
                current_playing,
            } = self;
            let model = model.lock().expect("no mutex poison");
            let (mut model, wait_result) = current_playing
                .wait_timeout_while(model, wait_timeout, |model| {
                    let next_track = model.get_available_next_track();
                    tracing::debug!(?next_track);
                    next_track.is_none()
                })
                .expect("no mutex poison");

            if wait_result.timed_out() {
                return None;
            }

            #[expect(
                clippy::panic,
                reason = "violates Condvar::wait_timeout_white postcondition"
            )]
            let Some(next_track) = model.get_available_next_track() else {
                panic!("wait succeeded but no next track")
            };

            let id = next_track.id;
            let state = PlayState::Playing;
            tracing::debug!(?next_track);
            model.set_current_playing(id, state);

            Some(())
        }
        // Non-mutable accessors
        pub fn lock_with_model_ref<T>(&self, act_fn: impl FnOnce(&Model) -> T) -> T {
            let model = self.model.lock().expect("no mutex poison");
            act_fn(&model)
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

mod auth_result {
    pub enum AuthError {
        Missing,
        Incorrect { found: String, expected: String },
    }
    impl AuthError {
        pub fn from_request(
            request: &tiny_http::Request,
            expected_bearer: &str,
        ) -> Result<(), Self> {
            let Some(bearer) = request
                .headers()
                .iter()
                .find(|h| h.field.equiv("authorization"))
            else {
                return Err(Self::Missing);
            };
            let found = &bearer.value;
            if found != expected_bearer {
                return Err(Self::Incorrect {
                    found: found.to_string(),
                    expected: expected_bearer.to_string(),
                });
            }
            Ok(())
        }
        pub fn http_code(&self) -> u16 {
            const HTTP_401_UNAUTHORIZED: u16 = 401;
            const HTTP_403_FORBIDDEN: u16 = 403;
            match self {
                AuthError::Missing => HTTP_401_UNAUTHORIZED,
                AuthError::Incorrect { .. } => HTTP_403_FORBIDDEN,
            }
        }
    }
    impl std::fmt::Display for AuthError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                AuthError::Missing => write!(f, "bearer heading missing"),
                AuthError::Incorrect { found, expected } => write!(
                    f,
                    "bearer heading incorrect, expected {expected:?}, found {found:?}"
                ),
            }
        }
    }
}
