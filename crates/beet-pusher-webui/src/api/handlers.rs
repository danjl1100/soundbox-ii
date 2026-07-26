// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use axum::http::StatusCode;
use utoipa::OpenApi;

pub use self::node::node_routes;

mod node;

#[derive(OpenApi)]
#[openapi(
    paths(node::create_bucket),
    tags((name = "nodes", description = "Nodes in the bucket spigot graph"))
)]
pub(crate) struct ApiDoc;

pub async fn health_check() -> StatusCode {
    StatusCode::OK
}
