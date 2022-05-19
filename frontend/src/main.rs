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

use backoff::ExponentialBackoff;
use gloo_timers::callback::Interval;
use yew::prelude::*;

mod fmt;

mod svg;

const LOG_RENDERS: bool = false;
#[macro_use]
mod macros;

use controls::Controls;
mod controls;

use playback::{PlaybackMeta, PlaybackPosition};
mod playback;

mod websocket;

derive_wrapper! {
    #[allow(clippy::large_enum_variant)]
    enum Msg for Model {
        WebSocket(MsgWebSocket) for update_websocket(..),
        User(MsgUser) for update_user(..),
    }
}

#[allow(clippy::large_enum_variant)] //TODO is this valid?
enum MsgWebSocket {
    Connect,
    Notify(websocket::Notify),
    ReceiveMessage(shared::ServerResponse),
    ReceiveError(anyhow::Error),
}
enum MsgUser {
    SendCommand(shared::Command),
    ClearErrors,
    IntervalTick,
}

type WebsocketHelper =
    websocket::Helper<shared::ClientRequest, shared::ServerResponse, ExponentialBackoff>;
struct Model {
    websocket: WebsocketHelper,
    playback: Option<(shared::PlaybackStatus, shared::Time)>,
    errors: Vec<String>,
    location: web_sys::Location,
    _interval: Interval,
    link: ComponentLink<Self>,
}
impl Model {
    fn new(link: ComponentLink<Self>) -> Self {
        let location = web_sys::window().expect("window exists").location();
        link.send_message(MsgWebSocket::Connect);
        Self {
            websocket: Self::new_websocket(&link, &location),
            playback: None,
            errors: vec![],
            location,
            _interval: Self::new_interval_tick(&link),
            link,
        }
    }
    fn new_websocket(link: &ComponentLink<Self>, location: &web_sys::Location) -> WebsocketHelper {
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
    fn new_interval_tick(link: &ComponentLink<Self>) -> Interval {
        const INTERVAL_MS: u32 = 500;
        let callback = link.callback(|_| MsgUser::IntervalTick);
        Interval::new(INTERVAL_MS, move || {
            callback.emit(());
        })
    }
}
impl Model {
    fn view_disconnected(&self) -> Html {
        const RED: svg::Renderer = svg::Renderer {
            stroke: "none",
            fill: "#e13e3e", // "red",
        };
        let reconnect_msg = if self.websocket.is_started() {
            html! { {"Connecting..."}}
        } else {
            let auto_reconnect_status = self.websocket.get_reconnect_timeout_millis().map_or_else(
                || {
                    html! { {"Auto-reconnect has given up (not scheduled)."} }
                },
                |millis| {
                    let seconds = f64::from(millis) / 1000.0;
                    #[allow(clippy::cast_possible_truncation)]
                    #[allow(clippy::cast_sign_loss)]
                    let seconds = seconds.abs().trunc() as u64;
                    html! {
                        <>
                        { "Trying to reconnect in " }
                        { fmt::fmt_duration_seconds_long(seconds) }
                        </>
                    }
                },
            );
            html! {
                <div>
                    <span>{ auto_reconnect_status }</span>
                    <span>
                        <button onclick=self.link.callback(|_| MsgWebSocket::Connect)>
                            { "Reconnect Now" }
                        </button>
                    </span>
                </div>
            }
        };
        let svg_image = if self.websocket.is_before_first_connect() {
            html! {}
        } else {
            html! {
                <div class="disconnected col-s-2">
                    { RED.render(svg::X_CROSS) }
                </div>
            }
        };
        html! {
            <div class="row">
                { svg_image }
                <div class="disconnected col-s-10">
                    <span class="title">{ "Connecting to server..."}</span>
                    { reconnect_msg }
                </div>
            </div>
        }
    }
    fn view_connected(&self) -> Html {
        let heartbeat_str = if let Some(time) = self.websocket.last_heartbeat() {
            format!("Server last seen: {:?}", time)
        } else {
            "Server last seen: Never".to_string()
        };
        html! {
            <div>
                <div class="row">
                    { self.view_playback() }
                    { self.view_album_art() }
                </div>
                <p style="font-size: 0.7em;">{ heartbeat_str }</p>
                { self.view_errors() }
            </div>
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
        let image_src = format!("/v1/art?trick_reload_key={}", trick_reload_key);
        html! {
            <div class="playback art col-7 col-s-5">
                <img src=image_src alt="Album Art" />
            </div>
        }
    }
    fn view_playback(&self) -> Html {
        if let Some((playback, playback_received)) = &self.playback {
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
                        on_command=self.link.callback(MsgUser::SendCommand)
                        playback_state=playback_state
                        ty=ty
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
                            timing=playback.timing
                            received_time=*playback_received
                            on_command=self.link.callback(MsgUser::SendCommand)
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
    fn view_errors(&self) -> Html {
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
                    <button onclick=self.link.callback(|_| MsgUser::ClearErrors)>
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
    fn update_websocket(&mut self, msg: MsgWebSocket) -> ShouldRender {
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
    fn update_user(&mut self, msg: MsgUser) -> ShouldRender {
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
    fn create(_props: Self::Properties, link: ComponentLink<Self>) -> Self {
        Self::new(link)
    }
    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        msg.update_on(self)
    }
    fn change(&mut self, _props: Self::Properties) -> ShouldRender {
        // no props
        false
    }
    fn view(&self) -> Html {
        log_render!("Model");
        let content = if self.websocket.is_connected() {
            self.view_connected()
        } else {
            self.view_disconnected()
        };
        html! {
            <>
                <header class="monospace">{ "soundbox-ii" }</header>
                <div class="content">
                    { content }
                </div>
                <footer>{ "(c) 2021 - don't keep your sounds boxed up" }</footer>
            </>
        }
    }
}

fn main() {
    yew::start_app::<Model>();
}
