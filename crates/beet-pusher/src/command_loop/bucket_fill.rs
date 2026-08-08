// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use std::sync::mpsc::SendError;

use crate::{
    BeetCommand,
    beet::{BucketQueryNeed, BucketQueryResult},
    command_loop::queue,
};

use super::LoopEvent;

pub enum Response {
    Fill(BucketQueryResult<std::io::Error>),
    Complete,
}

pub struct Sender {
    queue_tx: queue::Tx<BucketQueryNeed>,
}
impl Sender {
    pub fn queue(&self, queries: Vec<BucketQueryNeed>) {
        self.queue_tx.set_values(queries);
    }
}

/// Spawns a thread to handle [`BucketQueryNeed`] requests
pub fn spawn(
    loop_tx: std::sync::mpsc::SyncSender<LoopEvent>,
    runner: BeetCommand<'static>,
) -> (Sender, std::thread::JoinHandle<()>) {
    let (queue_tx, mut queue_rx) = queue::channel();

    let sender = Sender { queue_tx };
    let handle = std::thread::spawn(move || {
        match run(&mut queue_rx, &loop_tx, runner) {
            Ok(()) => {}
            Err(SendError(_event)) => {
                // NOTE: nothing to do, main event loop is no longer receiving
            }
        }
    });
    (sender, handle)
}

fn run(
    current_queue: &mut queue::Rx<BucketQueryNeed>,
    loop_tx: &std::sync::mpsc::SyncSender<LoopEvent>,
    mut runner: BeetCommand<'static>,
) -> Result<(), SendError<LoopEvent>> {
    while let Some(elem) = current_queue.pop_blocking() {
        let queue::Popped {
            elem: need,
            is_last,
        } = elem;

        let result = need.query(&mut runner);

        loop_tx.send(LoopEvent::BucketFillResponse(Response::Fill(result)))?;

        if is_last {
            loop_tx.send(LoopEvent::BucketFillResponse(Response::Complete))?;
        }
    }
    Ok(())
}
