// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use beet_pusher::pipe_exec::{Command, ResponseData};

pub trait BeetPusherPipe: Send + Sync + 'static {
    type Error: std::error::Error + Send + Sync + 'static;
    fn send(
        &self,
        command: Command,
        timeout: std::time::Duration,
    ) -> impl Future<Output = Result<ResponseData, Self::Error>> + Send;
}
