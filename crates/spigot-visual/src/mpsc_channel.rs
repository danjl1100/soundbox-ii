// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Utilities for [`std::sync::mpsc`] channels

use std::time::Instant;

/// Spawns a thread and returns a [`std::sync::mpsc::SyncSender`] that accepts narrower inputs,
/// using the provided `map_fn` to widen back to send via the input sender
pub fn spawn_narrower_sender<T: Send + 'static, U: Send + 'static>(
    tx: std::sync::mpsc::SyncSender<U>,
    map_fn: impl Fn(T) -> U + Send + 'static,
) -> (std::sync::mpsc::SyncSender<T>, std::thread::JoinHandle<()>) {
    let (narrow_tx, narrow_rx) = std::sync::mpsc::sync_channel(0);

    let handle = std::thread::spawn(move || {
        while let Ok(value) = narrow_rx.recv() {
            let mapped = map_fn(value);

            let Ok(()) = tx.send(mapped) else {
                break;
            };
        }
    });
    (narrow_tx, handle)
}

/// Spawns a thread and returns a [`std::sync::mpsc::SyncSender`] that accepts narrower inputs,
/// using the provided `map_fn` to widen back to send via the input sender
///
/// # Errors
/// Returns an error if a send fails
pub fn heartbeat_sender<T: Send + 'static>(
    tx: &std::sync::mpsc::SyncSender<T>,
    interval: std::time::Duration,
    mut heartbeat_fn: impl FnMut() -> T + Send + 'static,
) -> Result<std::convert::Infallible, std::sync::mpsc::SendError<T>> {
    let mut overslept = std::time::Duration::default();
    loop {
        overslept = {
            let start = Instant::now();

            // shorten the sleep by how much we overshot last time
            let remaining_to_sleep = interval.saturating_sub(overslept);
            std::thread::sleep(remaining_to_sleep);

            start.elapsed().saturating_sub(remaining_to_sleep)
        };

        // NOTE: respect back-pressure
        // if channel is full, then that pause the heartbeat interval schedule
        match tx.send(heartbeat_fn()) {
            Ok(()) => {}
            Err(e) => break Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::heartbeat_sender;
    use std::time::Instant;

    #[test]
    fn steady_heartbeat_interval_10() {
        steady_heartbeat_interval(20, 10);
    }
    #[test]
    fn steady_heartbeat_interval_20() {
        steady_heartbeat_interval(10, 20);
    }
    #[test]
    fn steady_heartbeat_interval_30() {
        steady_heartbeat_interval(7, 30);
    }
    #[test]
    fn steady_heartbeat_interval_40() {
        steady_heartbeat_interval(5, 40);
    }
    fn steady_heartbeat_interval(count: usize, interval: u64) {
        let tolerance_ms = cfg_select! {
            target_os = "macos" => 1 + (interval / 2),
            _ => 1,
        };

        let (tx, rx) = std::sync::mpsc::sync_channel(0);
        let handle = {
            let interval = std::time::Duration::from_millis(interval);
            std::thread::spawn(move || heartbeat_sender(&tx, interval, || ()))
        };

        let mut intervals_recv = Vec::with_capacity(count);
        for _ in 0..count {
            rx.recv().expect("pushes heartbeat");
            intervals_recv.push(Instant::now());
        }
        drop(rx);

        let Err(_) = handle.join().expect("sender should not panic");

        // assertions
        {
            assert_eq!(intervals_recv.len(), count);

            let deltas: Vec<_> = intervals_recv
                .array_windows()
                .map(|[prev, next]| {
                    let delta = next.duration_since(*prev);
                    delta.as_millis()
                })
                .collect();

            assert!(
                deltas.iter().all(|d| {
                    let min = (interval - tolerance_ms).into();
                    let max = (interval + tolerance_ms).into();
                    (min..=max).contains(d)
                }),
                "interval={interval} deltas={deltas:#?}"
            );
        }
    }
}
