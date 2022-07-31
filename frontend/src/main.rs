// soundbox-ii/frontend music playback controller *don't keep your sounds boxed up*
// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.
//
//! Frontend (JS) client

// TODO: only while building
#![allow(dead_code)]
// teach me
#![deny(clippy::pedantic)]
// no unsafe
#![forbid(unsafe_code)]
// no unwrap
#![deny(clippy::unwrap_used)]
// no panic
#![deny(clippy::panic)]
// docs!
#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

use backoff::{exponential::ExponentialBackoff, SystemClock};
use gloo_net::websocket::WebSocketError;
use yew::{html, Component, Context, Html};
use yew_router::{
    prelude::{Link, Redirect},
    BrowserRouter, Routable, Switch,
};

const LOG_RENDERS: bool = false;
#[macro_use]
mod macros;

mod old_main; //TODO deleteme

#[derive(Clone, Routable, PartialEq)]
enum Route {
    #[at("/")]
    Root,
    #[at("/player")]
    Player,
    #[not_found]
    #[at("/404")]
    NotFound,
}
impl Route {
    fn default() -> Self {
        Self::Player
    }
}

fn switch_main(route: &Route) -> Html {
    match route {
        Route::Root => html! {
            <Redirect<Route> to={Route::default()} />
        },
        Route::Player => html! {<h3>{"Player"}</h3>},
        Route::NotFound => html! {
            <>
                <h3>{"Not Found :\\"}</h3>
                <Link<Route> to={Route::default()}>{"Back to Home"}</Link<Route>>
            </>
        },
    }
}

derive_wrapper! {
    #[allow(clippy::large_enum_variant)]
    enum AppMsg for App {
        WebSocket(websocket::Msg) for self.websocket,
        Reconnect(reconnect::Msg) for self.reconnect,
        Logger(log::Msg) for self.logger,
    }
}
mod log {
    use yew::{html, Component, Context, Html};

    use crate::macros::UpdateDelegate;

    shared::wrapper_enum! {
        pub(crate) enum Msg {
            ClearErrors(ClearErrors),
            { impl None for }
            Message(String),
            Error((&'static str, String)),
            ShowErrorDetails(bool),
        }
    }
    pub struct ClearErrors {
        _seal: (),
    }
    #[derive(Default)]
    pub(crate) struct Logger {
        errors: Vec<LogErrorEntry>,
        show_error_details: bool,
    }
    impl Logger {
        pub(crate) fn error_view<C>(&self, ctx: &Context<C>) -> Html
        where
            C: Component,
            <C as Component>::Message: From<Msg>,
        {
            let show_details = self.show_error_details;
            let link = ctx.link();
            let toggle_details = link.callback(move |_| Msg::ShowErrorDetails(!show_details));
            let clear = link.callback(|_| Msg::from(ClearErrors { _seal: () }));
            html! {
                <div>
                    if self.errors.is_empty() {
                        { "No Errors" }
                    } else {
                        <>
                            { self.errors.len() }
                            { " Errors " }
                            <button onclick={toggle_details}>
                                { if show_details { "Hide" } else { "Show" } }
                            </button>
                            { " " }
                            <button onclick={clear}>{ "Clear" }</button>
                            if show_details {
                                { self.errors.iter().collect::<Html>() }
                            }
                        </>
                    }
                </div>
            }
        }
        fn clear_errors(&mut self) -> bool {
            let was_empty = self.errors.is_empty();
            self.errors.clear();
            !was_empty
        }
        fn set_show_error_details(&mut self, show: bool) -> bool {
            let changed = self.show_error_details != show;
            self.show_error_details = show;
            changed
        }
    }
    struct LogErrorEntry(&'static str, String);
    impl<'a> FromIterator<&'a LogErrorEntry> for Html {
        fn from_iter<T: IntoIterator<Item = &'a LogErrorEntry>>(iter: T) -> Self {
            html! {
                <ul class="errors">
                    {
                        iter.into_iter().enumerate().map(|(index, LogErrorEntry(ty, msg))| html! {
                            <li key={index}>
                                <b>{ty}</b>
                                {" - "}
                                { msg }
                            </li>
                        }).collect::<Html>()
                    }
                </ul>
            }
        }
    }
    impl<C> UpdateDelegate<C> for Logger
    where
        C: Component,
    {
        type Message = Msg;

        fn update(&mut self, _ctx: &Context<C>, message: Self::Message) -> bool {
            match message {
                Msg::Message(message) => {
                    log!("App got message {}", message);
                    false
                }
                Msg::Error((error_ty, error_msg)) => {
                    self.errors.push(LogErrorEntry(error_ty, error_msg));
                    true
                }
                Msg::ClearErrors(..) => {
                    let errors = self.clear_errors();
                    let show = self.set_show_error_details(false);
                    errors || show
                }
                Msg::ShowErrorDetails(show) => self.set_show_error_details(show),
            }
        }
    }
}

type WebsocketHandler = websocket::Handler<shared::ServerResponse, shared::ClientRequest>;
struct App {
    websocket: WebsocketHandler,
    reconnect: reconnect::Logic<ExponentialBackoff<SystemClock>>,
    logger: log::Logger,
}
impl App {
    fn new_websocket(ctx: &Context<Self>) -> WebsocketHandler {
        let url_websocket = {
            let location = web_sys::window().expect("window exists").location();
            let host = location.host().expect("window.location has host");
            format!("ws://{host}/ws")
        };
        let link = ctx.link();
        let on_message =
            link.callback(|server_response| log::Msg::Message(format!("{server_response:?}")));
        let on_error = link.callback(|e| -> AppMsg {
            const TYPE_WEBSOCKET: &str = "websocket";
            const TYPE_SERDE: &str = "serde";
            match e {
                websocket::Error::WebSocket(e) => match e {
                    WebSocketError::ConnectionError | WebSocketError::MessageSendError(_) => {
                        reconnect::Msg::ConnectionError.into()
                    }
                    WebSocketError::ConnectionClose(_) => reconnect::Msg::ConnectionClose.into(),
                    other => log::Msg::Error((
                        TYPE_WEBSOCKET,
                        format!("unknown WebSocketError: {other:?}"),
                    ))
                    .into(),
                },
                websocket::Error::UnexpectedBytes(bytes) => {
                    log::Msg::Error((TYPE_WEBSOCKET, format!("unexpected bytes: {bytes:?}"))).into()
                }
                websocket::Error::SerdeJson(error) => {
                    log::Msg::Error((TYPE_SERDE, format!("{error}"))).into()
                }
            }
        });
        websocket::Handler::new(
            url_websocket,
            websocket::Callbacks {
                on_message,
                on_error,
            },
        )
    }
    fn new_reconnect_backoff() -> backoff::ExponentialBackoff {
        use std::time::Duration;
        ExponentialBackoff {
            current_interval: Duration::from_secs(2),
            initial_interval: Duration::from_secs(2),
            multiplier: 2.0,
            max_interval: Duration::from_secs(20 * 60),
            max_elapsed_time: Some(Duration::from_secs(30 * 60)),
            ..ExponentialBackoff::default()
        }
    }
}
impl Component for App {
    type Message = AppMsg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let link = ctx.link();
        let websocket = Self::new_websocket(ctx);
        let reconnect = {
            let backoff = Self::new_reconnect_backoff();
            let connect = link.callback(|_| websocket::Msg::Connect);
            let disconnect = link.callback(|_| websocket::Msg::Disconnect);
            reconnect::Logic::new(
                backoff,
                reconnect::Callbacks {
                    connect,
                    disconnect,
                },
            )
        };
        Self {
            websocket,
            reconnect,
            logger: log::Logger::default(),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, message: AppMsg) -> bool {
        if self.reconnect.get_timeout_millis().is_some() {
            if let AppMsg::Logger(log::Msg::Message(..)) = &message {
                ctx.link()
                    .callback(|_| reconnect::Msg::ConnectionEstablished)
                    .emit(());
            }
        }
        message.update_on(self, ctx)
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let link = ctx.link();
        let websocket_connect = link.callback(|_| websocket::Msg::Connect);
        let websocket_disconnect = link.callback(|_| websocket::Msg::Disconnect);
        html! {
            <>
                <header class="monospace">{ "soundbox-ii" }</header>
                <div class="content">
                    <div>
                        {"This is a websocket test"}
                        <br/>
                        {"Websocket "}
                        <button onclick={websocket_connect}>{"Connect"}</button>
                        <button onclick={websocket_disconnect}>{"Disconnect"}</button>
                    </div>
                    <BrowserRouter>
                        <Switch<Route> render={Switch::render(switch_main)} />
                    </BrowserRouter>
                    { self.logger.error_view(ctx) }
                </div>
                <footer>{ "(c) 2021-2022 - don't keep your sounds boxed up" }</footer>
            </>
        }
    }
}

mod websocket {
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
}
mod reconnect {
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
}

fn main() {
    yew::start_app::<App>();
}
