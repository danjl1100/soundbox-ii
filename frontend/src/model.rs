// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use shared::ServerResponse;
use yew::{Callback, Component, Context};

use crate::macros::UpdateDelegate;

pub enum Msg {
    Server(ServerResponse),
}
#[derive(Debug)]
pub enum Error {
    ServerError(String),
}

pub struct Model {
    pub callbacks: Callbacks,
    pub data: Data,
}
#[derive(Default, Clone, PartialEq)]
pub struct Data {
    last_heartbeat: Option<shared::Time>,
    playback: Option<(shared::PlaybackStatus, shared::Time)>,
}
pub struct Callbacks {
    pub on_error: Callback<Error>,
    pub reload_page: Callback<()>,
}
impl Model {
    pub fn new(callbacks: Callbacks) -> Self {
        Self {
            data: Data::default(),
            callbacks,
        }
    }
}
impl Data {
    pub fn playback_status(&self) -> Option<&(shared::PlaybackStatus, shared::Time)> {
        self.playback.as_ref()
    }
    pub fn playback_info(&self) -> Option<&shared::PlaybackInfo> {
        self.playback
            .as_ref()
            .and_then(|(status, _)| status.information.as_ref())
    }
    pub fn last_heartbeat(&self) -> Option<shared::Time> {
        self.last_heartbeat
    }
}

impl<C> UpdateDelegate<C> for Model
where
    C: Component,
{
    type Message = Msg;

    fn update(&mut self, _ctx: &Context<C>, message: Self::Message) -> bool {
        match message {
            Msg::Server(message) => {
                log!("Server message: {message:?}");
                match message {
                    ServerResponse::Heartbeat | ServerResponse::Success => {}
                    ServerResponse::ClientCodeChanged => {
                        self.callbacks.reload_page.emit(());
                    }
                    ServerResponse::ServerError(err) => {
                        self.callbacks.on_error.emit(Error::ServerError(err));
                    }
                    ServerResponse::PlaybackStatus(playback) => {
                        let now = shared::time_now();
                        self.data.playback.replace((playback, now));
                    }
                }
                self.data.last_heartbeat.replace(shared::time_now());
                true // always, due to Heartbeat
            }
        }
    }
}
