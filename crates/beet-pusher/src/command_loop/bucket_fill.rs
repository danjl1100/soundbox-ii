// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use crossbeam_channel::SendError;

use crate::{
    BeetCommand,
    beet::{BucketQueryNeed, BucketQueryResult},
    command_loop::queue,
};

use super::LoopEvent;

#[derive(Debug)]
pub enum Response {
    Fill(BucketQueryResult<std::io::Error>),
    End,
}

pub struct Sender {
    queue_tx: queue::Tx<Vec<BucketQueryNeed>>,
}
impl Sender {
    pub fn queue(&self, queries: Vec<BucketQueryNeed>) {
        self.queue_tx.set_value(queries);
    }
}

/// Spawns a thread to handle [`BucketQueryNeed`] requests
pub fn spawn(
    loop_tx: crossbeam_channel::Sender<LoopEvent>,
    runner: BeetCommand<'static>,
) -> (Sender, JoinHandle) {
    let (queue_tx, mut queue_rx) = queue::channel(vec![]);

    let sender = Sender { queue_tx };
    let handle = std::thread::spawn(move || {
        match run(&mut queue_rx, &loop_tx, runner) {
            Ok(()) => {}
            Err(SendError(_event)) => {
                // NOTE: nothing to do, main event loop is no longer receiving
            }
        }
    });
    (sender, JoinHandle(handle))
}

fn run(
    current_queue: &mut queue::Rx<Vec<BucketQueryNeed>>,
    loop_tx: &crossbeam_channel::Sender<LoopEvent>,
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
            loop_tx.send(LoopEvent::BucketFillResponse(Response::End))?;
        }
    }
    Ok(())
}

pub struct JoinHandle(std::thread::JoinHandle<()>);
impl JoinHandle {
    /// Joins the thread, consuming the associated sender to avoid a deadlock
    pub fn join_and_drop(self, sender: Sender) -> std::thread::Result<()> {
        drop(sender);

        let Self(handle) = self;
        handle.join()
    }
}
