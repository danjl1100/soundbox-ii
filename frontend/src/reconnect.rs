// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use backoff::backoff::Backoff;
use gloo_timers::callback::Timeout;
use yew::{Callback, Component, Context};

use crate::macros::UpdateDelegate;

pub enum Msg {
    ConnectionEstablished,
    ConnectionClose,
    ConnectionError,
}
pub struct Callbacks {
    pub connect: Callback<()>,
    pub disconnect: Callback<()>,
}

pub struct Logic<B: Backoff> {
    timeout_millis: Option<(Timeout, u32)>,
    backoff: B,
    callbacks: Callbacks,
}
impl<B: Backoff> Logic<B> {
    pub fn new(backoff: B, callbacks: Callbacks) -> Self {
        Self {
            timeout_millis: None,
            backoff,
            callbacks,
        }
    }
    /// Clears the current timeout (persists the backoff state)
    fn clear_timeout(&mut self) {
        log!("reconnect: clear timeout");
        self.timeout_millis = None;
    }
    /// Resets the backoff and timeout
    fn reset_all(&mut self) {
        self.clear_timeout();
        self.backoff.reset();
    }
    /// Returns the duration of the current scheduled timeout, in milliseconds
    pub fn get_timeout_millis(&self) -> Option<u32> {
        self.timeout_millis.as_ref().map(|(_, millis)| *millis)
    }
    /// Schedules the next connect timeout
    fn schedule_timeout(&mut self) {
        if self.timeout_millis.is_some() {
            log!("reconnect: ignore schedule timeout, already scheduled");
            return;
        }
        if let Some(delay) = self.backoff.next_backoff() {
            log!("reconnect: schedule timeout for {delay:?}");
            let delay_millis = delay.as_millis().try_into().unwrap_or(u32::MAX);
            debug!("reconnect delay_millis = {}", delay_millis);
            let connect = self.callbacks.connect.clone();
            let timeout = Timeout::new(delay_millis, move || {
                connect.emit(());
            });
            self.timeout_millis = Some((timeout, delay_millis));
        }
    }
}
impl<B: Backoff, C: Component> UpdateDelegate<C> for Logic<B> {
    type Message = Msg;

    fn update(&mut self, _ctx: &Context<C>, message: Self::Message) -> bool {
        match message {
            Msg::ConnectionEstablished => self.reset_all(), // DONE
            Msg::ConnectionClose => self.schedule_timeout(), // only if needed, Retry (??)
            Msg::ConnectionError => {
                self.clear_timeout();
                self.schedule_timeout(); // RETRY
            }
        }
        true
    }
}
