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
use yew_router::BrowserRouter;

const LOG_RENDERS: bool = false;
#[macro_use]
mod macros;

// general-purpose utilities
mod fmt;
mod svg;
mod colors {
    pub const NONE: &str = "none";
    pub const RED: &str = "#e13e3e";
    pub const BLACK: &str = "black";
}

// update delegates
mod log;
mod reconnect;
mod websocket;

mod old_main; //TODO deleteme

// domain-specific
mod model;
mod router;
mod view {
    pub use disconnected::Disconnected;
    mod disconnected;

    pub use heartbeat::Heartbeat;
    mod heartbeat {
        use std::borrow::Cow;
        use yew::{function_component, html, html::IntoPropValue, Properties};

        use crate::{model, router};

        #[derive(PartialEq)]
        pub struct Data {
            last_heartbeat: Option<shared::Time>,
        }
        #[derive(Properties, PartialEq)]
        pub struct Props {
            pub data: Data,
            pub show_debug: bool,
        }
        impl IntoPropValue<Data> for &model::Data {
            fn into_prop_value(self) -> Data {
                let last_heartbeat = self.last_heartbeat();
                Data { last_heartbeat }
            }
        }

        #[function_component(Heartbeat)]
        pub fn heartbeat(props: &Props) -> Html {
            let Data { last_heartbeat } = props.data;
            html! {
                <div>
                    if props.show_debug {
                        <>
                            <router::Link to={router::Route::DebugPanel}>
                                { "Debug" }
                            </router::Link>
                            { " " }
                        </>
                    }
                    { "Sever last seen: " }
                    { last_heartbeat.map_or(Cow::Borrowed("Never"), |t| format!("{t:?}").into()) }
                </div>
            }
        }
    }

    pub use album_art::AlbumArt;
    mod album_art {
        use yew::{function_component, html, html::IntoPropValue, Properties};

        #[derive(PartialEq)]
        pub struct Data {
            hash: u64,
        }
        #[derive(Properties, PartialEq)]
        pub struct Props {
            pub data: Data,
        }
        impl IntoPropValue<Data> for Option<&shared::PlaybackInfo> {
            fn into_prop_value(self) -> Data {
                // NOTE: less-attractive alternative: store all fields in props, and defer
                // calculating hash until after `yew` PartialEq verifies the fields are different,
                // in the `view` function.    (the current implementation seems best)
                if self.is_none() {
                    log!("AlbumArt given prop data {self:?}");
                }
                let hash = self.map_or(0, |info| {
                    use std::hash::Hasher;
                    let mut hasher = twox_hash::XxHash64::with_seed(0);
                    let fields = [
                        &info.title,
                        &info.artist,
                        &info.album,
                        &info.date,
                        &info.track_number,
                    ];
                    log!("AlbumArt fields are: {fields:?}");
                    for (idx, field) in fields.iter().enumerate() {
                        hasher.write(field.as_bytes());
                        hasher.write_usize(idx);
                    }
                    hasher.finish()
                });
                Data { hash }
            }
        }

        #[function_component(AlbumArt)]
        pub fn album_art(Props { data }: &Props) -> Html {
            let Data { hash } = data;
            let src = format!("/v1/art?trick_reload_key={hash}");
            html! {
                <img {src} alt="Album Art" class="keep-true-color" />
            }
        }
    }

    pub use playback::Playback;
    mod playback {
        use std::rc::Rc;
        use yew::{function_component, html, html::IntoPropValue, Callback, Properties};

        #[derive(PartialEq)]
        struct DataInner {
            playback_status: Rc<shared::PlaybackStatus>,
            received_time: shared::Time,
        }
        #[derive(PartialEq)]
        pub struct Data(Option<DataInner>);
        #[derive(Properties, PartialEq)]
        pub struct Props {
            pub data: Data,
            pub on_command_opt: Callback<Option<shared::Command>>,
        }
        impl IntoPropValue<Data> for Option<&(shared::PlaybackStatus, shared::Time)> {
            fn into_prop_value(self) -> Data {
                Data(self.map(|(playback, received_time)| DataInner {
                    received_time: *received_time,
                    playback_status: Rc::new(playback.clone()),
                }))
            }
        }

        #[function_component(Playback)]
        pub fn playback(props: &Props) -> Html {
            if let Data(Some(data)) = &props.data {
                let on_command_opt = &props.on_command_opt;
                let on_command = on_command_opt.reform(Option::Some);
                let DataInner {
                    playback_status,
                    received_time,
                } = data;
                let playback_timing = playback_status.timing;
                let playback_timing_state = playback_status.timing.state;
                let volume_str = format!("{}%", playback_status.volume_percent);
                html! {
                    <>
                        <div class="playback control">
                            <controls::TrackPause
                                on_command={on_command.clone()}
                                {playback_timing_state}
                                />
                        </div>
                        <div class="playback meta">
                            <info::PlaybackMeta {playback_status} />
                            <timing::PlaybackPosition
                                {playback_timing}
                                received_time={*received_time}
                                {on_command_opt}
                                />
                            <div class="playback control">
                                <span>
                                    <label>{ "Seek" }</label>
                                    <controls::Seek on_command={on_command.clone()} />
                                </span>
                                <span>
                                    <label>{ "Volume" }</label>
                                    <controls::Volume {on_command} />
                                    <label>{ volume_str }</label>
                                </span>
                            </div>
                        </div>
                    </>
                }
            } else {
                html! { "No playback status... yet." }
            }
        }

        mod controls {
            use crate::{colors, svg};
            use shared::Command;
            use yew::{function_component, html, Callback, Html, Properties};

            const LABEL_PREVIOUS: (&str, &svg::Def) = ("Previous", svg::PREV);
            const LABEL_NEXT: (&str, &svg::Def) = ("Next", svg::NEXT);
            const LABEL_PLAY: (&str, &svg::Def) = ("Play", svg::PLAY);
            const LABEL_PAUSE: (&str, &svg::Def) = ("Pause", svg::PAUSE);
            const LABEL_FORWARD: (&str, &svg::Def) = ("Forward", svg::FORWARD);
            const LABEL_BACKWARD: (&str, &svg::Def) = ("Backward", svg::BACKWARD);
            const LABEL_LOUDER: (&str, &svg::Def) = ("Louder", svg::PLUS);
            const LABEL_SOFTER: (&str, &svg::Def) = ("Softer", svg::MINUS);

            #[derive(Properties, PartialEq)]
            pub struct Props {
                pub on_command: Callback<shared::Command>,
                pub playback_timing_state: shared::PlaybackState,
            }
            #[derive(Properties, PartialEq)]
            pub struct CmdProps {
                pub on_command: Callback<shared::Command>,
            }

            #[function_component(TrackPause)]
            pub fn track_pause(props: &Props) -> Html {
                let Props {
                    on_command: cb,
                    playback_timing_state,
                } = props;
                let is_paused = *playback_timing_state == shared::PlaybackState::Paused;
                let is_playing = *playback_timing_state == shared::PlaybackState::Playing;
                html! {
                    <>
                        { button(cb, LABEL_PREVIOUS, Command::SeekPrevious) }
                        if !is_playing {
                            { button(cb, LABEL_PLAY, Command::PlaybackResume) }
                        }
                        if !is_paused {
                            { button(cb, LABEL_PAUSE, Command::PlaybackPause) }
                        }
                        { button(cb, LABEL_NEXT, Command::SeekNext) }
                    </>
                }
            }

            #[function_component(Seek)]
            pub fn seek(CmdProps { on_command: cb }: &CmdProps) -> Html {
                const SEEK_BACKWARD: shared::Command = Command::SeekRelative { seconds_delta: -5 };
                const SEEK_FORWARD: shared::Command = Command::SeekRelative { seconds_delta: 5 };
                html! {
                    <>
                        { button(cb, LABEL_BACKWARD, SEEK_BACKWARD) }
                        { button(cb, LABEL_FORWARD, SEEK_FORWARD) }
                    </>
                }
            }

            #[function_component(Volume)]
            pub fn volume(CmdProps { on_command: cb }: &CmdProps) -> Html {
                const VOL_DOWN: shared::Command = Command::VolumeRelative { percent_delta: -5 };
                const VOL_UP: shared::Command = Command::VolumeRelative { percent_delta: 5 };
                html! {
                    <>
                        { button(cb, LABEL_SOFTER, VOL_DOWN) }
                        { button(cb, LABEL_LOUDER, VOL_UP) }
                    </>
                }
            }

            fn button(
                on_command: &Callback<Command>,
                (text, svg_def): (&str, &svg::Def),
                cmd: Command,
            ) -> Html {
                const BLACK_FILL: svg::Renderer = svg::Renderer {
                    stroke: colors::NONE,
                    fill: colors::BLACK,
                };
                let onclick = on_command.reform(move |_| cmd.clone());
                html! {
                    <button {onclick}>
                        { BLACK_FILL.render(svg_def) }
                        { text }
                    </button>
                }
            }
        }

        mod info {
            use std::rc::Rc;
            use yew::{function_component, html, Properties};

            #[derive(Properties, PartialEq)]
            pub struct Props {
                pub playback_status: Rc<shared::PlaybackStatus>,
            }
            #[function_component(PlaybackMeta)]
            pub fn playback_meta(props: &Props) -> Html {
                const SEPARATOR: &str = " \u{2014} ";
                if let Some(info) = &props.playback_status.information {
                    let artist = if info.artist.is_empty() {
                        "[No Artist]"
                    } else {
                        &info.artist
                    };
                    let album = if info.album.is_empty() {
                        "[No Album]"
                    } else {
                        &info.album
                    };
                    html! {
                        <>
                            <div>
                                <span class="title">{ &info.title }</span>
                            </div>
                            <div>
                                <span>
                                    <span class="artist">{ artist }</span>
                                    { SEPARATOR }
                                    <span class="album">{ album }</span>
                                </span>
                            </div>
                        </>
                    }
                } else {
                    html! {}
                }
            }
        }

        mod timing {
            use crate::fmt;
            use web_sys::HtmlInputElement;
            use yew::{
                function_component, html, Callback, Component, Context, Html, NodeRef, Properties,
            };

            #[derive(Properties, PartialEq)]
            pub struct Props {
                pub playback_timing: shared::PlaybackTiming,
                pub received_time: shared::Time,
                pub on_command_opt: Callback<Option<shared::Command>>,
            }
            #[derive(Debug)]
            pub enum Msg {
                PreviewSeekInput { seconds: u32 },
            }
            pub struct PlaybackPosition {
                /// User-input value (for responsive UI before server acknowledge)
                preview_position_secs: Option<u64>,
                input_ref: NodeRef,
            }
            impl Component for PlaybackPosition {
                type Message = Msg;
                type Properties = Props;

                fn create(_ctx: &yew::Context<Self>) -> Self {
                    Self {
                        preview_position_secs: None,
                        input_ref: NodeRef::default(),
                    }
                }

                fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
                    match msg {
                        Msg::PreviewSeekInput { seconds } => {
                            self.preview_position_secs = Some(u64::from(seconds));
                            true
                        }
                    }
                }

                fn changed(&mut self, _ctx: &Context<Self>) -> bool {
                    self.preview_position_secs = None;
                    true
                }

                fn view(&self, ctx: &Context<Self>) -> Html {
                    let Props {
                        on_command_opt,
                        playback_timing,
                        received_time,
                    } = ctx.props();
                    let on_seek = {
                        let input_ref = self.input_ref.clone();
                        on_command_opt.reform(move |_| {
                            let seconds = parse_position_str(&input_ref)?;
                            Some(shared::Command::SeekTo { seconds })
                        })
                    };
                    let on_seek_preview = {
                        let input_ref = self.input_ref.clone();
                        ctx.link().batch_callback(move |_| {
                            let seconds = parse_position_str(&input_ref)?;
                            Some(Msg::PreviewSeekInput { seconds })
                        })
                    };
                    let Self {
                        preview_position_secs,
                        input_ref: slider_input_ref,
                    } = self;
                    html! {
                        <InnerPlaybackPosition
                            playback_timing={*playback_timing}
                            preview_position_secs={*preview_position_secs}
                            received_time={*received_time}
                            {on_seek}
                            {on_seek_preview}
                            {slider_input_ref}
                            />
                    }
                }
            }
            fn parse_position_str(input_elem: &NodeRef) -> Option<u32> {
                input_elem
                    .cast::<HtmlInputElement>()
                    .map(|elem| elem.value())
                    .and_then(|value| {
                        use std::str::FromStr;
                        u32::from_str(&value).ok()
                    })
            }

            #[derive(Properties, PartialEq)]
            struct InnerProps {
                playback_timing: shared::PlaybackTiming,
                received_time: shared::Time,
                preview_position_secs: Option<u64>,
                on_seek: Callback<web_sys::Event>,
                on_seek_preview: Callback<web_sys::InputEvent>,
                slider_input_ref: NodeRef,
            }
            impl InnerProps {
                fn calc_forecast_position_secs(&self) -> u64 {
                    let shared::PlaybackTiming { position_secs, .. } = self
                        .playback_timing
                        .predict_change(shared::time_now() - self.received_time);
                    position_secs
                }
            }
            #[function_component(InnerPlaybackPosition)]
            fn inner_playback_position(props: &InnerProps) -> Html {
                let InnerProps {
                    playback_timing: shared::PlaybackTiming { duration_secs, .. },
                    received_time: _, // used in forecast
                    preview_position_secs,
                    on_seek,
                    on_seek_preview,
                    slider_input_ref,
                } = props;
                let position_secs =
                    preview_position_secs.unwrap_or_else(|| props.calc_forecast_position_secs());
                let remaining_secs = duration_secs.saturating_sub(position_secs);
                let duration_str = format!("{duration_secs}");
                let position_str = format!("{position_secs}");
                let position_fmt = fmt::fmt_duration_seconds(position_secs);
                let remaining_fmt = fmt::fmt_duration_seconds(remaining_secs);
                {
                    // `yew_hooks` shenanigans to trigger re-render every second
                    let dummy_state = yew::use_state(|| ());
                    let touch_state = move || {
                        dummy_state.set(());
                    };
                    yew_hooks::use_interval(touch_state, 1000);
                }
                html! {
                    <div class="playback time">
                        { position_fmt }
                        <input type="range"
                            min="0" max={duration_str} value={position_str}
                            ref={slider_input_ref.clone()}
                            onchange={on_seek}
                            oninput={on_seek_preview}
                            />
                        { "-" }{ remaining_fmt }
                    </div>
                }
            }
        }
    }
}

type WebsocketMsg = websocket::Msg<shared::ClientRequest>;
derive_wrapper! {
    #[allow(clippy::large_enum_variant)]
    enum AppMsg for App {
        Logger(log::Msg) for self.logger,
        Reconnect(reconnect::Msg) for self.reconnect,
        Model(model::Msg) for self.model,
        WebSocket(WebsocketMsg) for self.websocket,
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
        // redirection spans two levels
        AppMsgFull::Main(inner.into())
    }
}
impl From<shared::Command> for AppMsgFull {
    fn from(message: shared::Command) -> Self {
        websocket::Msg::SendMessage(message.into()).into()
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
                // log::Msg::Message(format!("{server_response:?}")).into(), //TODO deleteme, remove `Message` printouts (avoid churn in yew update framework)
                model::Msg::Server(server_response).into(),
                reconnect::Msg::ConnectionEstablished.into(),
            ]
        });
        let on_error = link.batch_callback(|e| {
            const TYPE_WEBSOCKET: &str = "websocket";
            const TYPE_SERDE: &str = "serde";
            const TYPE_WEBSOCKET_CHANNEL: &str = "websocket-channel";
            match e {
                websocket::Error::WebSocket(e) => match e {
                    WebSocketError::ConnectionError => {
                        vec![
                            reconnect::Msg::ConnectionError.into(),
                            websocket::Msg::Disconnect.into(),
                        ]
                    }
                    WebSocketError::MessageSendError(_) => {
                        vec![reconnect::Msg::ConnectionError.into()]
                    }
                    WebSocketError::ConnectionClose(_) => {
                        vec![
                            reconnect::Msg::ConnectionClose.into(),
                            websocket::Msg::Disconnect.into(),
                        ]
                    }
                    other => vec![log::Msg::Error((
                        TYPE_WEBSOCKET,
                        format!("unknown WebSocketError error: {other:?}"),
                    ))
                    .into()],
                },
                websocket::Error::UnexpectedBytes(bytes) => {
                    vec![
                        log::Msg::Error((TYPE_WEBSOCKET, format!("unexpected bytes: {bytes:?}")))
                            .into(),
                    ]
                }
                websocket::Error::SerdeJson(error) => {
                    vec![log::Msg::Error((TYPE_SERDE, format!("{error}"))).into()]
                }
                websocket::Error::Send(error) => {
                    vec![log::Msg::Error((TYPE_WEBSOCKET_CHANNEL, format!("{error}"))).into()]
                }
            }
        });
        let on_unsent_message = link.callback(|msg| {
            log::Msg::Error((
                "app",
                format!("disconnected, unable to send message: {msg:?}"),
            ))
        });
        websocket::Handler::new(
            url_websocket,
            websocket::Callbacks {
                on_message,
                on_error,
                on_unsent_message,
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
            model::Model::new(model::Callbacks {
                on_error,
                reload_page,
            })
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
        let ticked_all = AppMsg::tick_all(self);
        match message {
            AppMsgFull::Main(main) => main.update_on(self, ctx, ticked_all),
            AppMsgFull::Intrinsic(intrinsic) => match intrinsic {
                AppMsgIntrinsic::ReloadPage => self.reload_page(ctx),
            },
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let link = ctx.link();
        let on_reconnect_now = link.callback(|_| websocket::Msg::Connect);
        html! {
            <>
                <header class="monospace">{ "soundbox-ii" }</header>
                <div class="content">
                    <view::Disconnected data={self} {on_reconnect_now}>
                        <BrowserRouter>
                            { router::Main::switch_elem(self.model.data.clone(), ctx) }
                            <div style="font-size: 0.8em;">
                                <view::Heartbeat data={&self.model.data} show_debug=true />
                                { self.logger.error_view(ctx) }
                            </div>
                        </BrowserRouter>
                    </view::Disconnected>
                </div>
                <footer>{ "(c) 2021-2022 - don't keep your sounds boxed up" }</footer>
            </>
        }
    }
}

fn main() {
    log!("--------------------START APP MAIN--------------------");
    yew::start_app::<App>();
}
