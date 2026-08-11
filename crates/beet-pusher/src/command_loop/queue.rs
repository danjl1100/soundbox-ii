// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Invariants:
//! - Dropping [`Tx`] cancels any in progress or future [`Rx::pop_blocking`]

use std::sync::{Arc, Condvar, Mutex, MutexGuard};

use crate::Shutdown;

/// Removed item from [`Rx::pop_blocking`]
pub struct Popped<T> {
    pub elem: T,
    /// `true` if the popped item is the last one (of the current set)
    pub is_last: bool,
}

pub fn channel<T>(value: T) -> (Tx<T>, Rx<T>) {
    let inner = Arc::new(Inner {
        value: Mutex::new(Some(value)),
        cvar: Condvar::new(),
    });
    (Tx(inner.clone()), Rx(inner))
}

/// Sender for bulk items
pub struct Tx<T>(Arc<Inner<T>>);
/// Receiver for individual elements (`T`) in the current set, only
pub struct Rx<T>(Arc<Inner<T>>);

struct Inner<T> {
    value: Mutex<Option<T>>,
    cvar: Condvar,
}

impl<T> Tx<T> {
    /// Replace the queued list with the specified values
    pub fn set_value(&self, new_value: T) {
        let Self(inner) = self;
        let mut lock = inner.lock_or_panic();

        if let Some(value) = lock.as_mut() {
            *value = new_value;
            inner.cvar.notify_all();
        }
    }
}

impl<T> Rx<Vec<T>> {
    /// Wait for the next item, returning `None` if logically destructed
    pub fn pop_blocking(&self) -> Option<Popped<T>> {
        let Self(inner) = self;

        // shutdown if `Tx` is dropped
        let check_shutdown = || (Arc::strong_count(inner) < 2).then_some(Shutdown);

        let mut lock = inner.lock_or_panic();

        while check_shutdown().is_none() {
            lock = inner
                .cvar
                .wait_while(lock, |opt_vec| {
                    // wait while vec is present (not destructed)
                    matches!(opt_vec, Some(vec) if vec.is_empty()) && check_shutdown().is_none()
                })
                .expect("no poison");

            let Some(vec) = lock.as_mut() else {
                break;
            };
            let Some(elem) = vec.pop() else {
                continue;
            };
            return Some(Popped {
                elem,
                is_last: vec.is_empty(),
            });
        }
        None
    }
}

impl<T> Inner<T> {
    fn lock_or_panic(&self) -> MutexGuard<'_, Option<T>> {
        self.value.lock().expect("no poison")
    }
    fn destruct(&self) {
        {
            let mut lock = self.lock_or_panic();
            *lock = None;
        }
        self.cvar.notify_all();
    }
}
impl<T> Drop for Tx<T> {
    fn drop(&mut self) {
        let Self(inner) = self;
        inner.destruct();
    }
}
impl<T> Drop for Rx<T> {
    fn drop(&mut self) {
        let Self(inner) = self;
        inner.destruct();
    }
}
