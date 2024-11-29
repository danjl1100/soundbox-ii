// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

//! Clunky thread-related utilities

use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

pub(crate) enum Never {}
/// Executes the `risky_fn`, but if it takes longer than `timeout` then calls `timeout_fn` which
/// must halt
pub(crate) fn run_with_timeout<T: Send>(
    risky_fn: impl FnOnce() -> T + Send,
    timeout: Duration,
    timeout_fn: impl FnOnce(Duration) -> Never,
) -> T {
    const WAIT_DURATION: Duration = Duration::from_millis(1);
    const MUTEX_POISONED: &str = "finish mutex should not be poisoned";

    std::thread::scope(|s| {
        let pair = Arc::new((Mutex::new(false), Condvar::new()));

        let start = Instant::now();

        let pair2 = pair.clone();
        let handle = std::thread::Builder::new()
            .name(format!(
                "{} (with timeout)",
                std::thread::current().name().unwrap_or("unknown")
            ))
            .spawn_scoped(s, move || {
                let result = risky_fn();

                let (lock, cvar) = &*pair2;
                let mut finished = lock.lock().expect(MUTEX_POISONED);
                *finished = true;
                cvar.notify_one();

                result
            })
            .expect("no null bytes in thread name");
        let (lock, cvar) = &*pair;
        let mut finished = lock.lock().expect(MUTEX_POISONED);
        // NOTE: panic causes `handle.is_finished` before Mutex set (propagated on `join` below)
        while !*finished && !handle.is_finished() {
            let elapsed = start.elapsed();
            if elapsed >= timeout {
                let _never: Never = timeout_fn(elapsed);
                unreachable!();
            }

            let (new_finished, _wait_result) = cvar
                .wait_timeout(finished, WAIT_DURATION)
                .expect(MUTEX_POISONED);
            finished = new_finished;
        }
        handle.join().expect("wrapped thread panicked")
    })
}
