// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use futures::{
    channel::{mpsc, oneshot},
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

pub(crate) enum Msg<U> {
    Connect,
    Disconnect,
    SendMessage(U),
}

shared::wrapper_enum! {
    #[derive(Debug)]
    pub enum Error {
        WebSocket(WebSocketError),
        SerdeJson(serde_json::Error),
        Send(mpsc::SendError),
        { impl None for }
        UnexpectedBytes(Vec<u8>),
    }
}

type HandleWrite = SplitSink<WebSocket, Message>;
type HandleRead = SplitStream<WebSocket>;
pub(crate) struct Handler<T, U> {
    url: String,
    write_handle: Option<(
        mpsc::UnboundedSender<String>,
        oneshot::Receiver<shared::Shutdown>,
    )>,
    callbacks: Callbacks<T, U>,
    _phantom: PhantomData<U>,
}
impl<T, U> Handler<T, U>
where
    T: DeserializeOwned + 'static,
    U: Serialize + 'static,
{
    pub fn new(url: String, callbacks: Callbacks<T, U>) -> Self {
        Self {
            url,
            callbacks,
            write_handle: None,
            _phantom: PhantomData,
        }
    }
    pub fn write_handle(&mut self) -> Option<WriteHandle<'_, T, U>> {
        self.write_handle.as_ref().map(|(write_tx, _)| {
            let write_tx = write_tx.clone();
            let callbacks = &self.callbacks;
            WriteHandle {
                write_tx,
                callbacks,
                _phantom: PhantomData,
            }
        })
    }
    fn try_connect(&mut self) -> Result<(), JsError> {
        let ws = WebSocket::open(&self.url)?;
        let (write, read) = ws.split();
        let (read_loop_shutdown_tx, read_loop_shutdown_rx) = oneshot::channel();
        let (write_tx, write_rx) = mpsc::unbounded();
        self.write_handle.replace((write_tx, read_loop_shutdown_rx));
        let callbacks = self.callbacks.clone();
        spawn_local(callbacks.run_writer_loop(write, write_rx));
        let callbacks = self.callbacks.clone();
        spawn_local(callbacks.run_reader_loop(read, read_loop_shutdown_tx));
        Ok(())
    }
    pub fn update_loop_health(&mut self) {
        if let Some((write_tx, read_loop_shutdown_rx)) = &mut self.write_handle {
            let read_loop_shutdown = match read_loop_shutdown_rx.try_recv() {
                Ok(Some(shared::Shutdown)) | Err(oneshot::Canceled) => true,
                Ok(None) => false,
            };
            if read_loop_shutdown || write_tx.is_closed() {
                log!("websocket detected read/write loop termination, destroying connection");
                self.write_handle.take();
            }
        }
    }
}
pub struct Callbacks<T, U> {
    pub on_message: Callback<T>,
    pub on_error: Callback<Error>,
    pub on_unsent_message: Callback<U>,
}
impl<T, U> Clone for Callbacks<T, U> {
    fn clone(&self) -> Self {
        Self {
            on_message: self.on_message.clone(),
            on_error: self.on_error.clone(),
            on_unsent_message: self.on_unsent_message.clone(),
        }
    }
}
impl<T, U> Callbacks<T, U>
where
    T: DeserializeOwned,
{
    async fn run_reader_loop(
        self,
        mut read: HandleRead,
        mut shutdown: oneshot::Sender<shared::Shutdown>,
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
    async fn run_writer_loop(
        self,
        mut writer: HandleWrite,
        mut write_rx: mpsc::UnboundedReceiver<String>,
    ) {
        while let Some(message_str) = write_rx.next().await {
            let send_result = writer.send(Message::Text(message_str)).await;
            if let Err(send_err) = send_result {
                self.handle_error(send_err.into());
            }
        }
        log!("websocket write loop: Handler context ended");
    }
}
impl<C: Component, T, U> UpdateDelegate<C> for Handler<T, U>
where
    T: DeserializeOwned + 'static,
    U: Serialize + 'static,
{
    type Message = Msg<U>;
    fn update(&mut self, _ctx: &Context<C>, message: Self::Message) -> bool {
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
            Msg::SendMessage(message) => {
                match self.write_handle() {
                    Some(mut writer) => writer.send(&message),
                    None => self.callbacks.on_unsent_message.emit(message),
                }
                false
            }
        }
    }
    fn tick_all(&mut self) {
        self.update_loop_health();
    }
}

pub(crate) struct WriteHandle<'a, T, U> {
    write_tx: mpsc::UnboundedSender<String>,
    callbacks: &'a Callbacks<T, U>,
    _phantom: PhantomData<U>,
}

impl<'a, T, U> WriteHandle<'a, T, U>
where
    U: Serialize + 'static,
{
    /// Attempts to send the `message` to the websocket server.
    ///
    /// # Errors
    /// Sends any errors encountered to the `on_error` callback.
    pub fn send(&mut self, message: &U) {
        match serde_json::to_string(message) {
            Ok(message_str) => {
                let mut write_tx = self.write_tx.clone();
                let on_error = self.callbacks.on_error.clone();
                spawn_local(async move {
                    if let Err(err) = write_tx.send(message_str).await {
                        on_error.emit(err.into());
                    }
                });
            }
            Err(err) => {
                self.callbacks.on_error.emit(err.into());
            }
        }
    }
}
