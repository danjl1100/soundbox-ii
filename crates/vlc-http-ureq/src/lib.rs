// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! HTTP runner using [`ureq`] for [`vlc_http`]
use std::{str::FromStr as _, time::Duration};
use vlc_http::{Auth, Endpoint, Response, sync::EndpointRequestor};

pub use ::ureq as ureq_crate;

type ResponseStrObserver = dyn FnMut(&str) + Send;
type ResponseObserver = dyn FnMut(&Response) + Send;

/// Fulfills [`Endpoint`]s using the [`ureq`] HTTP client library
pub struct HttpRunner {
    auth: Auth,
    observe_fn_responses_str: Option<Box<ResponseStrObserver>>,
    observe_fn_responses: Option<Box<ResponseObserver>>,
    timeout_global: Option<Duration>,
}
impl HttpRunner {
    /// Creates a default with the specified [`Auth`]
    pub fn new(auth: Auth) -> Self {
        Self {
            auth,
            observe_fn_responses: None,
            observe_fn_responses_str: None,
            timeout_global: None,
        }
    }
    /// Allows custom logging of the raw HTTP response string, called for each endpoint
    ///
    /// NOTE: Replaces the previous "responses str" observer function (if any)
    pub fn set_observe_responses_str(&mut self, f: Box<ResponseStrObserver>) -> &mut Self {
        self.observe_fn_responses_str = Some(f);
        self
    }
    /// Allows custom logging of the parsed VLC [`Response`], called for each endpoint
    ///
    /// NOTE: Replaces the previous "responses" observer function (if any)
    pub fn set_observe_responses(&mut self, f: Box<ResponseObserver>) -> &mut Self {
        self.observe_fn_responses = Some(f);
        self
    }
    /// Sets the global timeout for each request (see
    /// [`ureq::config::ConfigBuilder::timeout_global`])
    pub fn set_timeout_global(&mut self, timeout: Duration) -> &mut Self {
        self.timeout_global = Some(timeout);
        self
    }
}
impl EndpointRequestor for HttpRunner {
    type Error = Error;
    fn request(&mut self, endpoint: Endpoint) -> Result<Response, Self::Error> {
        let make_error = |kind| Error { kind };

        let request = endpoint.with_auth(&self.auth).build_http_request();

        let request = {
            let (parts, ()) = request.into_parts();
            let ureq::http::request::Parts {
                method,
                uri,
                headers,
                extensions,
                ..
            } = parts;

            let mut builder = if method == ureq::http::Method::GET {
                ureq::get(uri)
            // } else if method == ureq::http::Method::POST {
            //     ureq::post(uri)
            } else {
                unimplemented!("unknown method {method:?}")
            };
            let headers_mut = builder
                .headers_mut()
                .expect("ureq builder should allow headers");
            headers_mut.extend(headers);

            let extensions_mut = builder
                .extensions_mut()
                .expect("ureq builder should allow extensions");
            extensions_mut.extend(extensions);

            builder
        };

        let mut response = request
            .config()
            .timeout_global(self.timeout_global)
            .build()
            .call()
            .map_err(Box::new)
            .map_err(ErrorKind::RequestCall)
            .map_err(make_error)?;
        let response_body = response
            .body_mut()
            .read_to_string()
            .map_err(Box::new)
            .map_err(ErrorKind::ResponseBody)
            .map_err(make_error)?;

        if let Some(observe_fn) = &mut self.observe_fn_responses_str {
            observe_fn(&response_body);
        }

        let response = Response::from_str(&response_body)
            .map_err(ErrorKind::ResponseParse)
            .map_err(make_error)?;

        if let Some(observe_fn) = &mut self.observe_fn_responses {
            observe_fn(&response);
        }

        Ok(response)
    }
}

/// Error calling an HTTP endpoint using `ureq` and parsing the result
#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
}
#[derive(Debug)]
enum ErrorKind {
    RequestCall(Box<ureq::Error>),
    ResponseBody(Box<ureq::Error>),
    ResponseParse(vlc_http::response::ParseError),
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            ErrorKind::RequestCall(error) | ErrorKind::ResponseBody(error) => Some(error),
            ErrorKind::ResponseParse(error) => Some(error),
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { kind } = self;
        let description = match kind {
            ErrorKind::RequestCall(_) => "request call failed",
            ErrorKind::ResponseBody(_) => "response body failed",
            ErrorKind::ResponseParse(_) => "invalid response",
        };
        write!(f, "{description}")
    }
}
impl Error {
    /// If the error was caused by `ureq`, returns the inner [`ureq::Error`]
    #[must_use]
    pub fn try_as_ureq(&self) -> Option<&ureq::Error> {
        match &self.kind {
            ErrorKind::RequestCall(error) | ErrorKind::ResponseBody(error) => Some(error),
            ErrorKind::ResponseParse(_) => None,
        }
    }
}
