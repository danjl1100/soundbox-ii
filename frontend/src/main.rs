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

mod log;
mod reconnect;
mod websocket;

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

fn main() {
    yew::start_app::<App>();
}
