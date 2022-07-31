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
use wasm_bindgen::JsValue;
use web_sys::Location;
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
        Logger(log::Msg) for self.logger,
        Reconnect(reconnect::Msg) for self.reconnect,
        Model(model::Msg) for self.model,
        WebSocket(websocket::Msg) for self.websocket,
    }
}
enum AppMsgIntrinsic {
    ReloadPage,
}
shared::wrapper_enum! {
    enum AppMsgFull {
        Intrinsic(AppMsgIntrinsic),
        { impl None for }
        Main(AppMsg),
    }
}
impl<T> From<T> for AppMsgFull
where
    AppMsg: From<T>,
{
    fn from(inner: T) -> Self {
        AppMsgFull::Main(inner.into())
    }
}

type WebsocketHandler = websocket::Handler<shared::ServerResponse, shared::ClientRequest>;
struct App {
    logger: log::Logger,
    model: model::Model,
    reconnect: reconnect::Logic<ExponentialBackoff<SystemClock>>,
    websocket: WebsocketHandler,
    window: web_sys::Window,
}
impl App {
    fn new_websocket(ctx: &Context<Self>) -> WebsocketHandler {
        let url_websocket = {
            let location = web_sys::window().expect("window exists").location();
            let host = location.host().expect("window.location has host");
            format!("ws://{host}/ws")
        };
        let link = ctx.link();
        let on_message = link.batch_callback(|server_response| {
            vec![
                // log::Msg::Message(format!("{server_response:?}")).into(), //TODO deleteme, remove `Message` printouts (framework churn)
                model::Msg::Server(server_response).into(),
                reconnect::Msg::ConnectionEstablished.into(),
            ]
        });
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
    fn reload_page(&mut self, ctx: &Context<Self>) -> bool {
        fn do_reload(location: &Location) -> Result<(), JsValue> {
            let location_str = location.href()?;
            location.set_href(&location_str)?;
            Ok(())
        }
        let location = self.window.location();
        self.reconnect.set_is_shutdown(true);
        let mut retry_count = 0;
        let link = ctx.link();
        while let Err(err) = do_reload(&location) {
            log::emit_error(link, "app", format!("page reload failed: {err:?}"));
            retry_count += 1;
            if retry_count > 10 {
                let bail_message = "page reload failed too many times :/".to_string();
                log::emit_error(link, "app", bail_message);
                break;
            }
        }
        false
    }
}
impl Component for App {
    type Message = AppMsgFull;
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
        let model = {
            let on_error =
                link.callback(|e| log::Msg::Error(("model", format!("TODO - handle this? {e:?}"))));
            let reload_page = link.callback(|()| AppMsgIntrinsic::ReloadPage);
            model::Model {
                status: model::Status::default(),
                callbacks: model::Callbacks {
                    on_error,
                    reload_page,
                },
            }
        };
        let window = web_sys::window().expect("JS has window");
        // startup WebSocket
        link.callback_once(|()| websocket::Msg::Connect).emit(());
        Self {
            logger: log::Logger::default(),
            reconnect,
            model,
            websocket,
            window,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, message: AppMsgFull) -> bool {
        match message {
            AppMsgFull::Main(main) => main.update_on(self, ctx),
            AppMsgFull::Intrinsic(intrinsic) => match intrinsic {
                AppMsgIntrinsic::ReloadPage => self.reload_page(ctx),
            },
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let link = ctx.link();
        let websocket_connect = link.callback(|_| websocket::Msg::Connect);
        let websocket_disconnect = link.callback(|_| websocket::Msg::Disconnect);
        let fake_error = link.callback(|_| log::Msg::Error(("debug", "fake error".to_string())));
        html! {
            <>
                <header class="monospace">{ "soundbox-ii" }</header>
                <div class="content">
                    <button onclick={fake_error}>{ "Trigger fake error" }</button>
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
                    <div style="font-size: 0.8em;">
                        { self.model.status.heartbeat_view() }
                        { self.logger.error_view(ctx) }
                    </div>
                </div>
                <footer>{ "(c) 2021-2022 - don't keep your sounds boxed up" }</footer>
            </>
        }
    }
}

mod model {
    use std::borrow::Cow;

    use shared::ServerResponse;
    use yew::{html, Callback, Component, Context, Html};

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
        pub status: Status,
    }
    #[derive(Default)]
    pub struct Status {
        last_heartbeat: Option<shared::Time>,
        playback: Option<shared::PlaybackStatus>,
    }
    pub struct Callbacks {
        pub on_error: Callback<Error>,
        pub reload_page: Callback<()>,
    }
    impl Status {
        pub fn heartbeat_view(&self) -> Html {
            html! {
                <div>
                    { "Sever last seen: " }
                    { self.last_heartbeat.map_or(Cow::Borrowed("Never"), |t| format!("{t:?}").into()) }
                </div>
            }
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
                            self.status.playback.replace(playback);
                        }
                    }
                    self.status.last_heartbeat.replace(shared::time_now());
                    true // always, due to Heartbeat
                }
            }
        }
    }
}

fn main() {
    log!("--------------------START APP MAIN--------------------");
    yew::start_app::<App>();
}
