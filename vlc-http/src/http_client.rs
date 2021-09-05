//! HTTP-Client specific functions

pub(crate) mod response {
    use crate::{command::TextResponseType, Error, PlaybackStatus, PlaylistInfo, Time};
    #[derive(Debug)]
    #[allow(clippy::large_enum_variant)]
    pub enum Typed {
        Playback(PlaybackStatus),
        Playlist(PlaylistInfo),
    }

    pub async fn try_parse_body_text(
        response: Result<(TextResponseType, hyper::Response<hyper::Body>), hyper::Error>,
    ) -> Result<Typed, Error> {
        let now = chrono::Utc::now();
        match response {
            Ok((TextResponseType::Status, response)) => {
                parse_typed_body(response, PlaybackStatus::from_slice, now)
                    .await
                    .map(Typed::Playback)
            }
            Ok((TextResponseType::Playlist, response)) => {
                parse_typed_body(response, PlaylistInfo::from_slice, now)
                    .await
                    .map(Typed::Playlist)
            }
            Err(e) => Err(e.into()),
        }
    }
    async fn parse_typed_body<F, T, E>(
        response: hyper::Response<hyper::Body>,
        map_fn: F,
        now: Time,
    ) -> Result<T, Error>
    where
        F: FnOnce(&[u8], Time) -> Result<T, E>,
        Error: From<E>,
    {
        hyper::body::to_bytes(response.into_body())
            .await
            .map_err(|err| err.into())
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
    use crate::{auth::Credentials, request::RequestInfo};
    use hyper::{
        body::Body, client::Builder as ClientBuilder, Client as HyperClient,
        Request as HyperRequest,
    };
    type Client = HyperClient<hyper::client::connect::HttpConnector, Body>;
    type Request = HyperRequest<Body>;

    /// Execution context for [`RequestIntent`]s
    pub(crate) struct Context(Client, Credentials);
    impl Context {
        pub fn new(credentials: Credentials) -> Self {
            let client = ClientBuilder::default().build_http();
            Self(client, credentials)
        }
        pub async fn run<'a, 'b, T>(
            &self,
            request_intent: T,
        ) -> Result<hyper::Response<Body>, hyper::Error>
        where
            RequestInfo: From<T>,
        {
            let request_info = RequestInfo::from(request_intent);
            Ok(self.run_retry_loop(request_info).await?)
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
                Ok(self.0.request(request).await?)
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
