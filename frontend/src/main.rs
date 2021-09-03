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
type Time = chrono::DateTime<chrono::offset::Utc>;

mod fmt;

const LOG_RENDERS: bool = false;
#[macro_use]
mod macros;

use playback::{PlaybackMeta, PlaybackPosition, PositionInfo};
mod playback;

mod websocket;

derive_wrapper! {
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
    playback: Option<(shared::PlaybackStatus, Time)>,
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
        let reconnect_btn = |label| {
            html! {
                <button onclick=self.link.callback(|_| MsgWebSocket::Connect)>
                    { label }
                </button>
            }
        };
        let reconnect_msg = if self.websocket.is_started() {
            html! { {"Connecting..."}}
        } else if let Some(millis) = self.websocket.get_reconnect_timeout_millis() {
            let seconds = f64::from(millis) / 1000.0;
            #[allow(clippy::cast_possible_truncation)]
            #[allow(clippy::cast_sign_loss)]
            let seconds = seconds.abs().trunc() as u64;
            html! {
                <p>
                    { "Trying to reconnect in " }
                    { fmt::fmt_duration_seconds_long(seconds) }
                    <br/>
                    { reconnect_btn("Reconnect Now") }
                </p>
            }
        } else {
            reconnect_btn("Reconnect")
        };
        html! {
            <div>
                <h3>{ "Connecting to server..."}</h3>
                { reconnect_msg }
            </div>
        }
    }
    fn view_connected(&self) -> Html {
        let heartbeat_str = format!("Server last seen: {:?}", self.websocket.last_heartbeat());
        html! {
            <div>
                <NumFetcher />
                { self.view_playback() }
                <br/>
                <p style="font-size: 0.7em;">{ heartbeat_str }</p>
                { self.view_errors() }
                { self.view_album_art() }
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
            <div class="playback art">
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
            html! {
                <div>
                    { meta_html }
                    <PlaybackPosition
                        position_info=PositionInfo::from((playback, playback_received))
                        on_command=self.link.callback(MsgUser::SendCommand)
                        />
                    <Controls
                        on_command=self.link.callback(MsgUser::SendCommand)
                        playback_state=self.playback.as_ref().map(|(playback, _)| playback.state)
                        />
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
                        let now = chrono::Utc::now();
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

use controls::Controls;
mod controls {
    use shared::Command;
    use yew::prelude::*;

    macro_rules! svg_paths {
        (
            struct $struct:ident { path, width, height }
            $(
                const $name:ident = {
                    [ $width:expr , $height:expr $(,)?];
                    $(
                        $cmd:tt $($arg:expr),*
                    );+ $(;)?
                };
            )+
        ) => {
            struct $struct {
                path: &'static str,
                width: &'static str,
                height: &'static str,
                view_box: &'static str,
            }
            $(
                const $name : $struct = $struct {
                    path: concat!(
                        $(
                            svg_paths!(@path_d $cmd $($arg),*)
                        ),+
                    ),
                    width: stringify!($width),
                    height: stringify!($height),
                    view_box: svg_paths!(@str_cat 0, 0, $width, $height),
                };
            )+
        };
        (@path_d $cmd:tt $($arg:expr),*) => {
            concat!(
                svg_paths!(@valid_cmd $cmd $($arg),*),
                svg_paths!(@str_cat $cmd, $($arg),*)
            )
        };
        (@str_cat $($arg:tt),+ $(,)?) => {
            concat!( $( " ", stringify!($arg)),+ )
        };
        (@valid_cmd M $_x:expr, $_y:expr) => { "" };
        (@valid_cmd l $_x:expr, $_y:expr) => { "" };
        (@valid_cmd h $_x:expr) => { "" };
        (@valid_cmd v $_y:expr) => { "" };
        (@valid_cmd z) => { "" };
    }

    svg_paths! {
        struct SvgDef { path, width, height }
        const SVG_PLAY = {
            [12, 12]; // 1-10, with extra margin
            // triangle (x,y = 1-10)
            M 1, 1;
            v 10;
            l 10, -5;
            z;
        };
        const SVG_PAUSE = {
            [10, 10]; // 1-8, with extra margin
            // left box (x = 1-4)
            M 1, 1;
            v 8;
            h 3;
            v -8;
            z;
            // right box (x = 6-9)
            M 6, 1;
            v 8;
            h 3;
            v -8;
            z;
        };
        const SVG_NEXT = {
            [16, 8]; // width 14, with extra margin, square axes
            // right-ward triangle 1
            M 1, 1;
            v 6;
            l 6, -3;
            z;
            // right-ward triangle 2
            M 7, 1;
            v 6;
            l 6, -3;
            z;
            // right-most bar
            M 13, 1;
            v 6;
            h 2;
            v -6;
            z;
        };
        const SVG_PREV = {
            [16, 8]; // width 14, with extra margin, square axes
            // left-ward triangle 1
            M 15, 1;
            v 6;
            l -6, -3;
            z;
            // left-ward triangle 2
            M 9, 1;
            v 6;
            l -6, -3;
            z;
            // left-most bar
            M 3, 1;
            v 6;
            h -2;
            v -6;
            z;
        };
        const SVG_EMPTY = {
            [1, 1];
            M 0, 0;
        };
    }

    struct SvgRenderer {
        stroke: &'static str,
        fill: &'static str,
    }
    impl SvgRenderer {
        fn render(&self, svg_def: &SvgDef) -> Html {
            html! {
                <svg viewBox=svg_def.view_box>
                    <path d=svg_def.path stroke=self.stroke fill=self.fill />
                </svg>
            }
        }
    }

    const LABEL_PREVIOUS: (&str, &SvgDef) = ("Previous", &SVG_PREV);
    const LABEL_NEXT: (&str, &SvgDef) = ("Next", &SVG_NEXT);
    const LABEL_PLAY: (&str, &SvgDef) = ("Play", &SVG_PLAY);
    const LABEL_PAUSE: (&str, &SvgDef) = ("Pause", &SVG_PAUSE);

    #[derive(Properties, Clone)]
    pub(crate) struct Properties {
        pub on_command: Callback<Command>,
        pub playback_state: Option<shared::PlaybackState>,
    }

    pub(crate) enum Msg {}

    pub(crate) struct Controls {
        on_command: Callback<Command>,
        link: ComponentLink<Self>,
        playback_state: Option<shared::PlaybackState>,
    }
    impl Controls {
        fn view_buttons(&self) -> Html {
            let is_paused = self.playback_state == Some(shared::PlaybackState::Paused);
            let is_playing = self.playback_state == Some(shared::PlaybackState::Playing);
            let fetch_button = |(text, svg_def), cmd: Command, enable| {
                const BLACK: SvgRenderer = SvgRenderer {
                    stroke: "none",
                    fill: "black",
                };
                let style = if enable { "" } else { "display: none;" };
                html! {
                    <button onclick=self.on_command.reform(move |_| cmd.clone()) style=style>
                        { BLACK.render(svg_def) }
                        { text }
                    </button>
                }
            };
            html! {
                <>
                    { fetch_button(LABEL_PREVIOUS, Command::SeekPrevious, true) }
                    { fetch_button(LABEL_PLAY, Command::PlaybackResume, !is_playing) }
                    { fetch_button(LABEL_PAUSE, Command::PlaybackPause, !is_paused) }
                    { fetch_button(LABEL_NEXT, Command::SeekNext, true) }
                </>
            }
        }
    }
    impl Component for Controls {
        type Message = Msg;
        type Properties = Properties;
        fn create(props: Self::Properties, link: ComponentLink<Self>) -> Self {
            let Properties {
                on_command,
                playback_state,
            } = props;
            Self {
                on_command,
                link,
                playback_state,
            }
        }
        fn update(&mut self, msg: Self::Message) -> ShouldRender {
            match msg {}
        }
        fn change(&mut self, props: Self::Properties) -> ShouldRender {
            let Properties {
                on_command,
                playback_state,
            } = props;
            self.on_command = on_command; // Callback's `PartialEq` implementation is empirically useless
            set_detect_change! {
                self.playback_state = playback_state;
            }
        }
        fn view(&self) -> Html {
            log_render!("Controls");
            html! {
                <div class="playback control">
                    { self.view_buttons() }
                </div>
            }
        }
    }
}

use num_fetcher::NumFetcher;
mod num_fetcher {
    use yew::format::{Json, Nothing};
    use yew::prelude::*;
    use yew::services::fetch::{FetchService, FetchTask, Request, Response};

    pub enum Msg {
        GetNumber,
        ReceiveResponse(Result<shared::Number, anyhow::Error>),
        ClearNumber,
    }

    pub struct NumFetcher {
        fetch_task: Option<FetchTask>,
        num: Option<shared::Number>,
        link: ComponentLink<Self>,
        error: Option<String>,
    }
    impl NumFetcher {
        fn view_number(&self) -> Html {
            match &self.num {
                Some(num) => {
                    html! {
                        <>
                            <p>
                                <button onclick=self.link.callback(|_| Msg::ClearNumber)>
                                    { "Clear" }
                                </button>
                            </p>
                            <label>{ "Number:" }</label>
                            <p>{ format!("{:?}", num) }</p>
                        </>
                    }
                }
                None => {
                    html! {
                        <button onclick=self.link.callback(|_| Msg::GetNumber)>
                            { "What is the best number?" }
                        </button>
                    }
                }
            }
        }
        fn view_fetching(&self) -> Html {
            if self.fetch_task.is_some() {
                html! { <p>{ "Fetching data..." }</p> }
            } else {
                html! { <p></p> }
            }
        }
        fn view_error(&self) -> Html {
            if let Some(error) = &self.error {
                html! { <p>{ "Error: " }{ error.clone() }</p> }
            } else {
                html! { <p></p> }
            }
        }
    }
    impl Component for NumFetcher {
        type Message = Msg;
        type Properties = ();
        fn create(_props: Self::Properties, link: ComponentLink<Self>) -> Self {
            Self {
                fetch_task: None,
                num: None,
                link,
                error: None,
            }
        }
        fn change(&mut self, _props: Self::Properties) -> ShouldRender {
            false
        }
        fn update(&mut self, msg: Self::Message) -> ShouldRender {
            match msg {
                Msg::GetNumber => {
                    let request = Request::get("/v1/number")
                        .body(Nothing)
                        .expect("Could not build request.");
                    let callback = self.link.callback(
                        |response: Response<Json<Result<shared::Number, anyhow::Error>>>| {
                            let Json(data) = response.into_body();
                            Msg::ReceiveResponse(data)
                        },
                    );
                    let task =
                        FetchService::fetch(request, callback).expect("failed to start request");
                    self.fetch_task.replace(task);
                    // redraw
                    true
                }
                Msg::ReceiveResponse(response) => {
                    match response {
                        Ok(number) => {
                            self.num.replace(number);
                        }
                        Err(error) => {
                            self.error.replace(error.to_string());
                        }
                    }
                    self.fetch_task = None;
                    // redraw
                    true
                }
                Msg::ClearNumber => {
                    let prev = self.num.take();
                    // redraw
                    prev.is_some()
                }
            }
        }
        fn view(&self) -> Html {
            log_render!("NumFetcher");
            html! {
                <>
                    { self.view_fetching() }
                    { self.view_number() }
                    { self.view_error() }
                </>
            }
        }
    }
}

fn main() {
    yew::start_app::<Model>();
}
