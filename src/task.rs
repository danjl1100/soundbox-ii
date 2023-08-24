// Copyright (C) 2021-2023  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Utilities for managing concurrent tasks

use shared::{Never, Shutdown};
use tokio::sync::watch;

/// Receiver for the [`Shutdown`] signal
#[derive(Clone)]
pub struct ShutdownReceiver(watch::Receiver<Option<Shutdown>>);
impl ShutdownReceiver {
    /// Constructs a [`watch::Sender`] and [`ShutdownReceiver`] pair
    pub fn new() -> (watch::Sender<Option<Shutdown>>, Self) {
        let (tx, rx) = watch::channel(None);
        (tx, Self(rx))
    }
    /// Synchronous poll for Shutdown
    pub fn poll_shutdown(&self, task_name: &'static str) -> Option<Shutdown> {
        let value = *self.0.borrow();
        if let Some(Shutdown) = value {
            println!("{task_name} received shutdown");
        }
        value
    }
    /// Asynchronous poll for Shutdown
    pub async fn check_shutdown(&mut self, task_name: &'static str) -> Option<Shutdown> {
        let rx = &mut self.0;
        let changed_result = rx.changed().await;
        if changed_result.is_err() {
            eprintln!("error waiting for {task_name} shutdown signal, shutting down...");
            Some(Shutdown)
        } else {
            let shutdown = *rx.borrow();
            if shutdown.is_some() {
                println!("received shutdown: {task_name}");
            }
            shutdown
        }
    }
    /// Asynchronous wait for Shutdown
    pub async fn wait_for_shutdown(mut self, task_name: &'static str) {
        while self.check_shutdown(task_name).await.is_none() {
            continue;
        }
    }
}

pub struct AsyncTasks {
    handles: Vec<tokio::task::JoinHandle<()>>,
    shutdown_rx: ShutdownReceiver,
}
impl AsyncTasks {
    /// Creates an empty instance, using the specified [`ShutdownReceiver`] to abort tasks
    pub fn new(shutdown_rx: ShutdownReceiver) -> Self {
        Self {
            handles: vec![],
            shutdown_rx,
        }
    }
    /// Spawns a new async task, to be cancelled when Shutdown is received
    pub fn spawn(
        &mut self,
        task_name: &'static str,
        task: impl std::future::Future<Output = Result<Never, Shutdown>> + Send + 'static,
    ) {
        let mut shutdown_rx = self.shutdown_rx.clone();
        let handle = tokio::task::spawn(async move {
            tokio::select! {
                biased; // poll in-order (shutdown first)
                Some(Shutdown) = shutdown_rx.check_shutdown(task_name) => {}
                Err(Shutdown) = task => {}
                else => {}
            };
            println!("ended: {task_name}");
        });
        self.handles.push(handle);
    }
    /// Waits for all tasks to complete
    ///
    /// # Errors
    /// Returns an error if any task fails to join
    pub async fn join_all(self) -> Result<(), tokio::task::JoinError> {
        for task in self.handles {
            task.await?;
        }
        Ok(())
    }
}
