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

use gloo_timers::callback::Interval;
use yew::prelude::*;
type Time = chrono::DateTime<chrono::offset::Utc>;

mod websocket;

const LOG_RENDERS: bool = false;
#[macro_use]
mod macros;

use playback::{PlaybackMeta, PlaybackPosition, PositionInfo};
mod playback;

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

struct Model {
    link: ComponentLink<Self>,
    websocket: websocket::Helper<shared::ClientRequest, shared::ServerResponse>,
    playback: Option<(shared::PlaybackStatus, Time)>,
    errors: Vec<String>,
    _interval: Interval,
}
impl Model {
    fn view_disconnected(&self) -> Html {
        html! {
            <div>
                { "Disconnected from server, that's sad :/" }
                <br/>
                <button onclick=self.link.callback(|_| MsgWebSocket::Connect)>
                    { "Connect" }
                </button>
            </div>
        }
    }
    fn view_connected(&self) -> Html {
        html! {
            <div>
                <p>{ "This is generated in Yew!" }</p>
                <NumFetcher />
                { self.view_playback() }
                <Controls
                    on_command=self.link.callback(MsgUser::SendCommand)
                    playback_state=self.playback.as_ref().map(|(playback, _)| playback.state)
                    />
                <br/>
                { self.view_errors() }
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
                <>
                    { meta_html }
                    <PlaybackPosition
                        position_info=PositionInfo::from((playback, playback_received))
                        on_command=self.link.callback(MsgUser::SendCommand)
                        />
                </>
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
                log!("WEBSOCKET: Connecting...");
                match self.websocket.connect() {
                    Ok(_) => true,
                    Err(err) => {
                        self.push_error("websocket connect", err);
                        true
                    }
                }
            }
            MsgWebSocket::Notify(event) => {
                info!("WEBSOCKET: {:?}", event);
                self.websocket.on_notify(event)
            }
            MsgWebSocket::ReceiveMessage(message) => {
                log!("<- {:#?}", message);
                match message {
                    shared::ServerResponse::Success => false,
                    shared::ServerResponse::ServerError(err_message) => {
                        self.push_error("server", err_message);
                        true
                    }
                    shared::ServerResponse::PlaybackStatus(playback) => {
                        let now = chrono::Utc::now();
                        self.playback = Some((playback, now));
                        true
                    }
                }
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
fn create_websocket(
    link: &ComponentLink<Model>,
) -> websocket::Helper<shared::ClientRequest, shared::ServerResponse> {
    const URL_WEBSOCKET: &str = "ws://127.0.0.1:3030/ws"; //TODO: generalize this url... not always local!
    let on_message = link.callback(|msg| match msg {
        Ok(msg) => MsgWebSocket::ReceiveMessage(msg),
        Err(e) => MsgWebSocket::ReceiveError(e),
    });
    let on_notification = link.callback(MsgWebSocket::Notify);
    websocket::Helper::new(URL_WEBSOCKET, &on_message, on_notification)
}
impl Component for Model {
    type Message = Msg;
    type Properties = ();
    fn create(_props: Self::Properties, link: ComponentLink<Self>) -> Self {
        let websocket = create_websocket(&link);
        link.send_message(MsgWebSocket::Connect);
        let interval = {
            let callback = link.callback(|_| MsgUser::IntervalTick);
            Interval::new(500, move || {
                callback.emit(());
            })
        };
        Self {
            link,
            websocket,
            playback: None,
            errors: vec![],
            _interval: interval,
        }
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
        if self.websocket.is_connected() {
            self.view_connected()
        } else {
            self.view_disconnected()
        }
    }
}

use controls::Controls;
mod controls {
    use shared::Command;
    use yew::prelude::*;

    // reference table: https://stackoverflow.com/a/27053825/5742216
    const SYMBOL_PREVIOUS: &str = "\u{23EE}";
    const SYMBOL_NEXT: &str = "\u{23ED}";
    const SYMBOL_PLAY: &str = "\u{23F5}";
    const SYMBOL_PAUSE: &str = "\u{23F8}";

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
            //
            let fetch_button = |text, cmd: Command, enable| {
                if enable {
                    html! {
                        <button onclick=self.on_command.reform(move |_| cmd.clone())>
                            { text }
                        </button>
                    }
                } else {
                    html! {}
                }
            };
            html! {
                <>
                    { fetch_button(SYMBOL_PREVIOUS, Command::SeekPrevious, true) }
                    { fetch_button(SYMBOL_PLAY, Command::PlaybackResume, !is_playing) }
                    { fetch_button(SYMBOL_PAUSE, Command::PlaybackPause, !is_paused) }
                    { fetch_button(SYMBOL_NEXT, Command::SeekNext, true) }
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
