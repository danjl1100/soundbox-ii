// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! HTTP-level primitives (interchange for test purposes)

pub use endpoint::Endpoint;
mod endpoint;

// TODO
// pub struct AuthInfo {
//
// }
//
// /// HTTP request information to execute a [`Command`],
// /// complete with the server location and authentication details
// #[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
// pub struct RequestInfo {
//     endpoint: Endpoint,
//     auth: AuthInfo,
// }
// impl From<RequestInfo> for http::Request<()> {
//     fn from(value: RequestInfo) -> Self {
//         todo!()
//     }
// }
