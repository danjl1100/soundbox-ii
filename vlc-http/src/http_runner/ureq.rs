// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! HTTP runner using [`ureq`]
use crate::{sync::EndpointRequestor, Auth, Endpoint, Response};
use std::str::FromStr as _;

type ResponseStrObserver = dyn FnMut(&str);
type ResponseObserver = dyn FnMut(&Response);

/// Fulfills [`Endpoint`]s using the [`ureq`] HTTP client library
pub struct HttpRunner {
    auth: Auth,
    observe_fn_responses_str: Option<Box<ResponseStrObserver>>,
    observe_fn_responses: Option<Box<ResponseObserver>>,
}
impl HttpRunner {
    /// Creates a default with the specified [`Auth`]
    pub fn new(auth: Auth) -> Self {
        Self {
            auth,
            observe_fn_responses: None,
            observe_fn_responses_str: None,
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
}
impl EndpointRequestor for HttpRunner {
    type Error = Error;
    fn request(&mut self, endpoint: Endpoint) -> Result<Response, Self::Error> {
        let make_error = |kind| Error { kind };

        let request = endpoint.with_auth(&self.auth).build_http_request();

        let request = {
            let (parts, ()) = request.into_parts();
            ureq::Request::from(parts)
        };

        let response = request
            .call()
            .map_err(Box::new)
            .map_err(ErrorKind::RequestCall)
            .map_err(make_error)?;
        let response_body = response
            .into_string()
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
    ResponseBody(std::io::Error),
    ResponseParse(crate::response::ParseError),
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            ErrorKind::RequestCall(error) => Some(error),
            ErrorKind::ResponseBody(error) => Some(error),
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
