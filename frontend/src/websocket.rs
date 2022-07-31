// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use futures::{
    channel::oneshot,
    select,
    stream::{SplitSink, SplitStream},
    FutureExt, SinkExt, StreamExt,
};
use gloo::utils::errors::JsError;
use gloo_net::websocket::{futures::WebSocket, Message, WebSocketError};
use serde::{de::DeserializeOwned, Serialize};
use std::marker::PhantomData;
use wasm_bindgen_futures::spawn_local;
use yew::{Callback, Component, Context};

use crate::macros::UpdateDelegate;

pub(crate) enum Msg {
    Connect,
    Disconnect,
}

shared::wrapper_enum! {
    #[derive(Debug)]
    pub enum Error {
        WebSocket(WebSocketError),
        SerdeJson(serde_json::Error),
        { impl None for }
        UnexpectedBytes(Vec<u8>),
    }
}

type HandleWrite = SplitSink<WebSocket, Message>;
type HandleRead = SplitStream<WebSocket>;
pub(crate) struct Handler<T, U> {
    url: String,
    write_handle: Option<(HandleWrite, oneshot::Receiver<shared::Never>)>,
    callbacks: Callbacks<T>,
    _phantom: PhantomData<U>,
}
impl<T, U> Handler<T, U>
where
    T: DeserializeOwned + 'static,
{
    pub fn new(url: String, callbacks: Callbacks<T>) -> Self {
        Self {
            url,
            callbacks,
            write_handle: None,
            _phantom: PhantomData,
        }
    }
    fn try_connect(&mut self) -> Result<(), JsError> {
        let ws = WebSocket::open(&self.url)?;
        let (write, read) = ws.split();
        let (read_loop_tx, read_loop_rx) = oneshot::channel();
        self.write_handle.replace((write, read_loop_rx));
        let callbacks = self.callbacks.clone();
        spawn_local(callbacks.run_reader_loop(read, read_loop_tx));
        Ok(())
    }
}
pub struct Callbacks<T> {
    pub on_message: Callback<T>,
    pub on_error: Callback<Error>,
}
impl<T> Clone for Callbacks<T> {
    fn clone(&self) -> Self {
        Self {
            on_message: self.on_message.clone(),
            on_error: self.on_error.clone(),
        }
    }
}
impl<T> Callbacks<T>
where
    T: DeserializeOwned,
{
    async fn run_reader_loop(
        self,
        mut read: HandleRead,
        mut shutdown: oneshot::Sender<shared::Never>,
    ) {
        let mut shutdown = shutdown.cancellation().fuse();
        loop {
            //TODO move outside the loop, per guidelines
            let mut read = read.next().fuse();
            select! {
                read_result = read => match read_result {
                    Some(message) => match self.handle_message(message) {
                        Ok(()) => {}
                        Err(error) => self.handle_error(error),
                    }
                    None => {
                        log!("websocket read loop: server hung up :/");
                        break;
                    }
                },
                _ = shutdown => {
                    log!("websocket read loop: shutdown accepted");
                    break;
                }
            };
        }
        // TODO code smell... how to `.fuse()` outside the loop (per docs), while still getting messages?
        std::mem::forget(read);
    }
    fn handle_message(&self, message: Result<Message, WebSocketError>) -> Result<(), Error> {
        // log!("websocket read: {message:?}");
        match message? {
            Message::Text(text) => {
                let message = serde_json::from_str(&text)?;
                self.on_message.emit(message);
            }
            Message::Bytes(bytes) => log!(
                "websocket read loop: unsupported input of bytes {:?}",
                bytes
            ),
        }
        Ok(())
    }
    fn handle_error(&self, error: Error) {
        log!("websocket error: {error:?}");
        self.on_error.emit(error);
    }
}
impl<C: Component, T, U> UpdateDelegate<C> for Handler<T, U>
where
    T: DeserializeOwned + 'static,
{
    type Message = Msg;
    fn update(&mut self, _ctx: &Context<C>, message: Msg) -> bool {
        match message {
            Msg::Connect => {
                log!("this is connect request!");
                if let Err(err) = self.try_connect() {
                    log!("websocket error: {}", err);
                }
                true
            }
            Msg::Disconnect => {
                let changed = self.write_handle.take().is_some();
                if changed {
                    log!("disconnected");
                }
                changed
            }
        }
    }
}
