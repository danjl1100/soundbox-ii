// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! HTTP-Client specific functions

pub(crate) mod intent {
    use shared::Time;

    use crate::{
        request::RequestInfo,
        vlc_responses::{PlaybackStatus, PlaylistInfo},
    };

    pub trait FromSliceAtTime
    where
        Self: Sized,
    {
        fn from_slice(bytes: &[u8], received_time: Time) -> Result<Self, serde_json::Error>;
    }
    /// Plan for how to request a specific type of data
    pub(crate) trait Intent {
        type Output: FromSliceAtTime;
        fn get_request_info(&self) -> RequestInfo;
    }

    #[derive(Debug, PartialEq, Eq)]
    pub struct CmdArgs {
        pub command: &'static str,
        pub args: Vec<(&'static str, String)>,
    }

    #[derive(Debug, PartialEq, Eq)]
    pub(crate) struct StatusIntent(pub Option<CmdArgs>);
    #[derive(Debug, PartialEq, Eq)]
    pub(crate) struct PlaylistIntent(pub Option<CmdArgs>);
    impl Intent for StatusIntent {
        type Output = PlaybackStatus;
        fn get_request_info(&self) -> RequestInfo {
            self.into()
        }
    }
    impl Intent for PlaylistIntent {
        type Output = PlaylistInfo;
        fn get_request_info(&self) -> RequestInfo {
            self.into()
        }
    }

    #[must_use]
    #[derive(Debug, PartialEq, Eq)]
    pub(crate) enum TextIntent {
        Status(StatusIntent),
        Playlist(PlaylistIntent),
    }
    impl TextIntent {
        pub(crate) fn status(command: &'static str) -> Self {
            Self::Status(StatusIntent(Some(CmdArgs {
                command,
                args: vec![],
            })))
        }
    }
    impl From<StatusIntent> for TextIntent {
        fn from(inner: StatusIntent) -> Self {
            Self::Status(inner)
        }
    }
    impl From<PlaylistIntent> for TextIntent {
        fn from(inner: PlaylistIntent) -> Self {
            Self::Playlist(inner)
        }
    }

    /// Query for Album Artwork for the current item
    pub(crate) struct ArtRequestIntent {
        pub id: Option<String>,
    }
}

pub(crate) mod response {
    use super::intent::{FromSliceAtTime, Intent};
    use crate::Error;
    use crate::{PlaybackStatus, PlaylistInfo};
    use shared::Time;
    shared::wrapper_enum! {
        #[derive(Debug)]
        #[allow(clippy::large_enum_variant)]
        pub enum Typed {
            Playback(PlaybackStatus),
            Playlist(PlaylistInfo),
        }
    }

    pub(crate) async fn try_parse_body_text<T: Intent>(
        response: hyper::Response<hyper::Body>,
        now: Time,
    ) -> Result<T::Output, Error> {
        let map_fn = T::Output::from_slice;
        hyper::body::to_bytes(response.into_body())
            .await
            .map_err(Into::into)
            .and_then(|bytes| Ok(map_fn(&bytes, now)?))
    }

    pub async fn try_parse(
        result: Result<hyper::Response<hyper::Body>, hyper::Error>,
    ) -> Result<Result<hyper::Response<hyper::Body>, String>, hyper::Error> {
        match result {
            Ok(response) => {
                // DETECT plain-text error message
                match response.headers().get("content-type") {
                    Some(content_type) if content_type == "text/plain" => {
                        let body = response.into_body();
                        let content = hyper::body::to_bytes(body).await.map(|bytes| {
                            String::from_utf8(bytes.to_vec()).expect("valid utf8 from VLC")
                            //TODO can this become a Hyper error? no... :/
                        });
                        match content {
                            Ok(text) => Ok(Err(text)),
                            Err(e) => Err(e),
                        }
                    }
                    _ => Ok(Ok(response)),
                }
            }
            Err(e) => Err(e),
        }
    }
}

pub(crate) use context::Context;
mod context {
    use crate::{auth::Authorization, request::RequestInfo};
    use hyper::{
        body::Body, client::Builder as ClientBuilder, Client as HyperClient,
        Request as HyperRequest,
    };
    type Client = HyperClient<hyper::client::connect::HttpConnector, Body>;
    type Request = HyperRequest<Body>;

    /// Execution context for [`TextIntent`]s
    pub(crate) struct Context(Client, Authorization);
    impl Context {
        pub fn new(credentials: Authorization) -> Self {
            let client = ClientBuilder::default().build_http();
            Self(client, credentials)
        }
        pub async fn run<T>(&self, request_intent: T) -> Result<hyper::Response<Body>, hyper::Error>
        where
            RequestInfo: From<T>,
        {
            let request_info = RequestInfo::from(request_intent);
            self.run_retry_loop(request_info).await
        }
        async fn run_retry_loop(
            &self,
            request: RequestInfo,
        ) -> Result<hyper::Response<Body>, hyper::Error> {
            use backoff::ExponentialBackoff;
            use tokio::time::Duration;
            let backoff_config = ExponentialBackoff {
                current_interval: Duration::from_millis(50),
                initial_interval: Duration::from_millis(50),
                multiplier: 4.0,
                max_interval: Duration::from_secs(2),
                max_elapsed_time: Some(Duration::from_secs(10)),
                ..ExponentialBackoff::default()
            };
            backoff::future::retry(backoff_config, || async {
                let request = self.request_from(request.clone()); //TODO: avoid expensive clone?
                println!("  -> {}: {}", request.method(), request.uri());
                let result = self.0.request(request).await;
                match &result {
                    Ok(response) => println!("    <- {:?}", response.status()),
                    Err(error) => println!("  !! {:?}", error),
                }
                Ok(result?)
            })
            .await
        }
        fn request_from(&self, info: RequestInfo) -> Request {
            let RequestInfo {
                path_and_query,
                method,
            } = info;
            let uri = self
                .1
                .uri_builder()
                .path_and_query(path_and_query)
                .build()
                .expect("internally-generated URI is valid");
            self.1
                .request_builder()
                .uri(uri)
                .method(method)
                .body(Body::empty())
                .expect("internally-generated URI and Method is valid")
        }
    }
}
