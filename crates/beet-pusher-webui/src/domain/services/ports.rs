// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Controls (ports) the services require from the backend

use beet_pusher::pipe_exec::{Command, ResponseData};

/// Port to execute commands and queries on [`beet_pusher`] instance
pub trait BeetPusherPipe: Send + Sync + 'static {
    /// Error from the backend
    type Error: std::error::Error + Send + Sync + 'static;
    /// Executes [`beet_pusher`] commands
    fn send(
        &self,
        command: Command,
        timeout: std::time::Duration,
    ) -> impl Future<Output = Result<ResponseData, Self::Error>> + Send;
}
