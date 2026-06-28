// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use std::sync::Arc;

use crate::{
    config::Config,
    domain::services::{node_service::NodeService, ports::BeetPusherPipe},
};
use axum::extract::FromRef;

pub struct AppState<T> {
    pub config: Arc<Config>,
    pub node_service: Arc<NodeService<T>>,
}
impl<T: BeetPusherPipe> AppState<T> {
    #[must_use]
    pub fn new(config: Config, pipe: T) -> Self {
        Self {
            config: Arc::new(config),
            node_service: Arc::new(NodeService::new(pipe)),
        }
    }
}

// NOTE: No perfect derive... yet
impl<T> Clone for AppState<T> {
    fn clone(&self) -> Self {
        let Self {
            config,
            node_service,
        } = self;
        Self {
            config: config.clone(),
            node_service: node_service.clone(),
        }
    }
}

// NOTE: FromRef derive does not support generics
impl<T> FromRef<AppState<T>> for Arc<Config> {
    fn from_ref(input: &AppState<T>) -> Self {
        input.config.clone()
    }
}
impl<T> FromRef<AppState<T>> for Arc<NodeService<T>> {
    fn from_ref(input: &AppState<T>) -> Self {
        input.node_service.clone()
    }
}
