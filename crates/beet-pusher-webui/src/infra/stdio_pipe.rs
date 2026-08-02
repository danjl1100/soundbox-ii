// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Communication to a [`beet_pusher`] backend

use std::{
    collections::VecDeque,
    sync::atomic::{AtomicU64, Ordering},
};

use beet_pusher::pipe_exec::{CommandIn, RequestSequence};

use crate::{Shutdown, domain::services::ports::BeetPusherPipe};

type IoThread<T = ()> = std::thread::JoinHandle<std::io::Result<T>>;

/// Communication bridge to [`beet_pusher`] over stdin/stdout
#[must_use]
pub struct StdioPipe {
    next_seq: AtomicU64,
    cmd_tx: tokio::sync::mpsc::Sender<Queued<beet_pusher::pipe_exec::CommandIn>>,
}
type ResponseResult = Result<beet_pusher::pipe_exec::ResponseData, ()>;
struct Queued<T> {
    value: T,
    tx: tokio::sync::oneshot::Sender<ResponseResult>,
}
impl StdioPipe {
    /// Spawns stdout/stdin threads to send commands and receive responses
    pub fn spawn(shutdown_tx: tokio::sync::mpsc::Sender<Shutdown>) -> (Self, IoThread, IoThread) {
        let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel(1);
        let (waiting_tx, waiting_rx) = std::sync::mpsc::sync_channel(10);

        let this = Self {
            next_seq: AtomicU64::new(0),
            cmd_tx,
        };
        let stdout_handle = std::thread::spawn(move || {
            use std::io::Write as _;

            let mut main_rx = cmd_rx;
            let mut stdout = std::io::stdout().lock();
            while let Some(cmd_and_tx) = main_rx.blocking_recv() {
                let Queued { value: cmd, tx } = cmd_and_tx;

                let _ = waiting_tx.send(Queued { value: cmd.seq, tx });

                tracing::trace!(?cmd, "WRITE STDOUT");

                serde_json::to_writer(&mut stdout, &cmd)?;
                writeln!(&mut stdout)?;
            }
            Ok(())
        });

        let stdin_handle = std::thread::spawn(move || {
            let mut waiting_inflight = WaitingChannels::default();
            'outer: for line in std::io::stdin().lines() {
                let line = line?;

                tracing::trace!(?line, "READ");

                // cull waiting_inflight
                waiting_inflight.cull_expired();

                // fill waiting_inflight
                loop {
                    match waiting_rx.try_recv() {
                        Ok(waiting) => {
                            let seq = waiting.value;
                            tracing::trace!(?seq, "ADDED");
                            waiting_inflight.push(waiting);
                        }
                        Err(std::sync::mpsc::TryRecvError::Empty) => break,
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                            tracing::error!("stdin thread ending, stdout thread is gone");
                            break 'outer;
                        }
                    }
                }

                let response: beet_pusher::pipe_exec::ResponseOutDe = serde_json::from_str(&line)?;
                match response {
                    beet_pusher::pipe_exec::ResponseOutDe::Data { reply_to_seq, data } => {
                        if let Some(tx) = waiting_inflight.find_take_tx(reply_to_seq) {
                            let _ = tx.send(Ok(data));
                        } else {
                            tracing::error!(?data, "no waiting entry for response");
                        }
                    }
                    beet_pusher::pipe_exec::ResponseOutDe::Error(error) => {
                        let reply_to_seq = error.get_reply_to_seq();
                        let error = error.into_inner();

                        if let Some(seq) = reply_to_seq
                            && let Some(tx) = waiting_inflight.find_take_tx(seq)
                        {
                            let _ = tx.send(Err(()));
                        }
                        tracing::error!(?error);
                    }
                }
            }
            let _ = shutdown_tx.blocking_send(Shutdown);
            Ok(())
        });

        (this, stdout_handle, stdin_handle)
    }
}

#[derive(Default)]
struct WaitingChannels {
    list: VecDeque<Option<Queued<beet_pusher::pipe_exec::RequestSequence>>>,
}
impl WaitingChannels {
    fn cull_expired(&mut self) {
        let Self { list } = self;

        while !list.is_empty() {
            if let Some(Some(entry)) = list.front()
                && !entry.tx.is_closed()
            {
                break;
            }
            let _removed = list.pop_front();
        }
    }
    fn push(&mut self, channel: Queued<beet_pusher::pipe_exec::RequestSequence>) {
        self.list.push_back(Some(channel));
    }
    fn find_take_tx(
        &mut self,
        needle: RequestSequence,
    ) -> Option<tokio::sync::oneshot::Sender<ResponseResult>> {
        // NOTE: linear search, hopefully responses are mostly sequential?
        self.list.iter_mut().find_map(|entry| {
            let Queued { value: seq, .. } = entry.as_mut()?;

            if *seq != needle {
                return None;
            }

            let Queued { tx, .. } = entry.take().expect("matched some");
            Some(tx)
        })
    }
}

impl BeetPusherPipe for StdioPipe {
    type Error = PipeError;
    async fn send(
        &self,
        cmd: beet_pusher::pipe_exec::Command,
        timeout: std::time::Duration,
    ) -> Result<beet_pusher::pipe_exec::ResponseData, Self::Error> {
        let seq = self.next_seq.fetch_add(1, Ordering::Relaxed);
        let seq = beet_pusher::pipe_exec::RequestSequence::from(seq);

        tracing::trace!(?cmd, "QUEUE");

        let cmd = CommandIn { seq, cmd };
        let (tx, rx) = tokio::sync::oneshot::channel();

        let cmd_tx_deadline = Queued { value: cmd, tx };

        self.cmd_tx
            .send_timeout(cmd_tx_deadline, timeout)
            .await
            .map_err(PipeErrorInner::SendTimeout)?;

        let response = tokio::time::timeout(timeout, rx)
            .await
            .map_err(PipeErrorInner::RecvTimeout)?
            .map_err(PipeErrorInner::Recv)?;

        response.map_err(|()| PipeErrorInner::ResponseFailed.into())
    }
}
/// Error communicating over the [`StdioPipe`]
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct PipeError(#[from] PipeErrorInner);
#[derive(Debug, thiserror::Error)]
enum PipeErrorInner {
    #[error("timed out waiting to queue command")]
    SendTimeout(
        #[source]
        tokio::sync::mpsc::error::SendTimeoutError<Queued<beet_pusher::pipe_exec::CommandIn>>,
    ),
    #[error("failed to unqueue result")]
    Recv(#[source] tokio::sync::oneshot::error::RecvError),
    #[error("timed out waiting for the result")]
    RecvTimeout(tokio::time::error::Elapsed),
    /// Received response successfully, but the response was an error
    #[error("failure in response")]
    ResponseFailed,
}
