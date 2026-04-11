// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! HTTP-level request primitives (interchange for test purposes)

use crate::{Auth, http};
pub use endpoint::Endpoint;
mod endpoint;

impl Endpoint {
    /// Returns a description of the HTTP request to reach this endpoint with the specified authentication
    pub fn with_auth<'a>(&'a self, auth: &'a Auth) -> RequestInfo<'a> {
        RequestInfo {
            endpoint: self,
            bearer_credential_plaintext: &auth.bearer_credential_plaintext,
            authority: &auth.authority,
        }
    }
}

/// Borrowed information to construct an HTTP request
#[expect(clippy::module_name_repetitions, reason = "name for un-scoped import")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequestInfo<'a> {
    /// destination path and query for the request
    endpoint: &'a Endpoint,
    /// Bearer string (base64 encoded password with prefix)
    bearer_credential_plaintext: &'a str,
    /// Host and Port
    authority: &'a http::uri::Authority,
}
impl RequestInfo<'_> {
    /// Creates an HTTP request
    ///
    /// # Panics
    /// Panics if the internal URI and request generation logic fails [`http`] valdiation checks
    #[must_use]
    pub fn build_http_request(self) -> http::Request<()> {
        const HEADER_AUTHORIZATION: &str = "Authorization";

        let Self {
            endpoint,
            bearer_credential_plaintext,
            authority,
        } = self;

        let uri = http::Uri::builder()
            .scheme("http")
            .authority(authority.clone())
            .path_and_query(endpoint.get_path_and_query())
            .build()
            .expect("internally-generated URI is valid");

        http::Request::builder()
            .header(HEADER_AUTHORIZATION, bearer_credential_plaintext)
            .uri(uri)
            .method(endpoint.get_method())
            .body(())
            .expect("internally-generated URI and Method is valid")
    }
}
