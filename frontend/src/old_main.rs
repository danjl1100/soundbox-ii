// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use backoff::ExponentialBackoff;
use gloo_timers::callback::Interval;
use yew::prelude::*;

use crate::{fmt, svg};

use controls::Controls;
mod controls;

use playback::{PlaybackMeta, PlaybackPosition};
mod playback;

mod websocket;

derive_wrapper! {
    #[allow(clippy::large_enum_variant)]
    pub(super) enum Msg for Model {
        WebSocket(MsgWebSocket) for update_websocket(..),
        User(MsgUser) for update_user(..),
    }
}

#[allow(clippy::large_enum_variant)] //TODO is this valid?
pub(super) enum MsgWebSocket {
    Connect,
    Notify(websocket::Notify),
    ReceiveMessage(shared::ServerResponse),
    ReceiveError(anyhow::Error),
}
pub(super) enum MsgUser {
    SendCommand(shared::Command),
    ClearErrors,
    IntervalTick,
}

type WebsocketHelper =
    websocket::Helper<shared::ClientRequest, shared::ServerResponse, ExponentialBackoff>;
pub(super) struct Model {
    websocket: WebsocketHelper,
    playback: Option<(shared::PlaybackStatus, shared::Time)>,
    errors: Vec<String>,
    location: web_sys::Location,
    _interval: Interval,
}
impl Model {
    fn new(ctx: &Context<Self>) -> Self {
        let location = web_sys::window().expect("window exists").location();
        ctx.link().send_message(MsgWebSocket::Connect);
        Self {
            websocket: Self::new_websocket(ctx, &location),
            playback: None,
            errors: vec![],
            location,
            _interval: Self::new_interval_tick(ctx),
        }
    }
    fn new_websocket(ctx: &Context<Self>, location: &web_sys::Location) -> WebsocketHelper {
        let link = ctx.link();
        let host = location.host().expect("window.location has host");
        let url_websocket = format!("ws://{}/ws", host);
        let on_message = link.callback(|msg| match msg {
            Ok(msg) => MsgWebSocket::ReceiveMessage(msg),
            Err(e) => MsgWebSocket::ReceiveError(e),
        });
        let on_notification = link.callback(MsgWebSocket::Notify);
        let on_reconnect = link.callback(|_| MsgWebSocket::Connect);
        let reconnect_backoff = Self::new_reconnect_backoff();
        WebsocketHelper::new(
            url_websocket,
            &on_message,
            on_notification,
            on_reconnect,
            reconnect_backoff,
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
    fn new_interval_tick(ctx: &Context<Self>) -> Interval {
        const INTERVAL_MS: u32 = 5000;
        let callback = ctx.link().callback(|_| MsgUser::IntervalTick);
        Interval::new(INTERVAL_MS, move || {
            callback.emit(());
        })
    }
}
impl Model {
    fn view_connected(&self, ctx: &Context<Self>) -> Html {
        let heartbeat_str = if let Some(time) = self.websocket.last_heartbeat() {
            format!("Server last seen: {:?}", time)
        } else {
            "Server last seen: Never".to_string()
        };
        let view_str = web_sys::window().expect("window exists").location().hash();
        match view_str.as_ref().map(String::as_str) {
            Ok("#special") => html! {
                <div>
                    { "This one is SPECIAL!" }
                </div>
            },
            _ => html! {
                <div>
                    <div class="row">
                        { self.view_playback(ctx) }
                        { self.view_album_art() }
                    </div>
                    <p style="font-size: 0.7em;">{ heartbeat_str }</p>
                    { self.view_errors(ctx) }
                </div>
            },
        }
    }
    fn view_album_art(&self) -> Html {
        let trick_reload_key = self
            .playback
            .as_ref()
            .and_then(|(playback, _)| playback.information.as_ref())
            .map_or(0, |info| {
                use std::hash::Hasher;
                let mut hasher = twox_hash::XxHash64::with_seed(0);
                let fields = [
                    &info.title,
                    &info.artist,
                    &info.album,
                    &info.date,
                    &info.track_number,
                ];
                for (idx, field) in fields.iter().enumerate() {
                    hasher.write(field.as_bytes());
                    hasher.write_usize(idx);
                }
                hasher.finish()
            });
        let src = format!("/v1/art?trick_reload_key={}", trick_reload_key);
        html! {
            <div class="playback art col-7 col-s-5">
                <img {src} alt="Album Art" />
            </div>
        }
    }
    fn view_playback(&self, ctx: &Context<Self>) -> Html {
        if let Some((playback, playback_received)) = &self.playback {
            let link = ctx.link();
            let meta_html = if let Some(info) = &playback.information {
                PlaybackMeta::render(info)
            } else {
                html! {}
            };
            let playback_state = self
                .playback
                .as_ref()
                .map(|(playback, _)| playback.timing.state);
            let controls = |ty| {
                html! {
                    <Controls
                        on_command={link.callback(MsgUser::SendCommand)}
                        {playback_state}
                        {ty}
                        />
                }
            };
            let volume_str = format!(
                "{}%",
                self.playback
                    .as_ref()
                    .map_or(0, |(playback, _)| playback.volume_percent)
            );
            html! {
                <div class="playback container col-5 col-s-7">
                    <div class="playback control">
                        { controls(controls::Type::TrackPause) }
                    </div>
                    <div class="playback meta">
                        { meta_html }
                        <PlaybackPosition
                            timing={playback.timing}
                            received_time={*playback_received}
                            on_command={link.callback(MsgUser::SendCommand)}
                            />
                        <div class="playback control">
                            <span>
                                <label>{"Seek"}</label>
                                { controls(controls::Type::Seek) }
                            </span>
                            <span>
                                <label>{"Volume"}</label>
                                { controls(controls::Type::Volume) }
                                <label>{ volume_str }</label>
                            </span>
                        </div>
                    </div>
                </div>
            }
        } else {
            html! { "No playback status... yet." }
        }
    }
    fn view_errors(&self, ctx: &Context<Self>) -> Html {
        let render_error = |err| {
            html! {
                <li>{ err }</li>
            }
        };
        if self.errors.is_empty() {
            html! {
                <div>
                    { "Errors: None" }
                </div>
            }
        } else {
            html! {
                <div>
                    { "Errors: " }
                    <button onclick={ctx.link().callback(|_| MsgUser::ClearErrors)}>
                        { "Clear" }
                    </button>
                    <ul>
                    { for self.errors.iter().map(render_error) }
                    </ul>
                </div>
            }
        }
    }
}
impl Model {
    fn push_error<E: std::fmt::Display>(&mut self, err_type: &str, error: E) {
        self.errors.push(format!("{} error: {}", err_type, error));
    }
    fn update_websocket(&mut self, _ctx: &Context<Self>, msg: MsgWebSocket) -> bool {
        match msg {
            MsgWebSocket::Connect => {
                if self.websocket.is_started() {
                    error!("refusing to connect websocket, already connected");
                    false
                } else {
                    log!("WEBSOCKET: Connecting...");
                    match self.websocket.connect() {
                        Ok(_) => true,
                        Err(err) => {
                            self.push_error("websocket connect", err);
                            true
                        }
                    }
                }
            }
            MsgWebSocket::Notify(event) => {
                info!("WEBSOCKET: {:?}", event);
                self.websocket.on_notify(event)
            }
            MsgWebSocket::ReceiveMessage(message) => {
                log!("<- {:#?}", message);
                self.websocket.on_message();
                match message {
                    shared::ServerResponse::Heartbeat | shared::ServerResponse::Success => {}
                    shared::ServerResponse::ClientCodeChanged => {
                        let reload_result = self.location.reload();
                        if let Err(reload_err) = reload_result {
                            error!("Error reloading: {:?}", reload_err);
                        }
                    }
                    shared::ServerResponse::ServerError(err_message) => {
                        self.push_error("server", err_message);
                        //true
                    }
                    shared::ServerResponse::PlaybackStatus(playback) => {
                        let now = shared::time_now();
                        self.playback = Some((playback, now));
                        //true
                    }
                }
                true
            }
            MsgWebSocket::ReceiveError(err) => {
                error!("ERROR: {:?}", err);
                self.push_error("receive", err);
                true
            }
        }
    }
    fn update_user(&mut self, _ctx: &Context<Self>, msg: MsgUser) -> bool {
        match msg {
            MsgUser::SendCommand(command) => {
                let payload = shared::ClientRequest::Command(command);
                log!("-> {:?}", &payload);
                if let Some(task) = self.websocket.get_task() {
                    task.send(&payload);
                }
                //true
                false
            }
            MsgUser::ClearErrors => {
                let was_empty = self.errors.is_empty();
                self.errors.clear();
                !was_empty
            }
            MsgUser::IntervalTick => true,
        }
    }
}
impl Component for Model {
    type Message = Msg;
    type Properties = ();
    fn create(ctx: &Context<Self>) -> Self {
        Self::new(ctx)
    }
    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        msg.update_on(self, ctx)
    }
    fn view(&self, ctx: &Context<Self>) -> Html {
        log_render!("Model");
        let content = if self.websocket.is_connected() {
            self.view_connected(ctx)
        } else {
            html! {}
        };
        html! {
            <>
                <header class="monospace">{ "soundbox-ii" }</header>
                <div class="content">
                    { content }
                </div>
                <p>
                    { "This is some live content, cool!" }
                    <br/>
                    { format!("{:?}", web_sys::window().expect("window exists").location().hash()) }
                </p>
                <footer>{ "(c) 2021 - don't keep your sounds boxed up" }</footer>
            </>
        }
    }
}
