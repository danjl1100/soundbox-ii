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

use yew::prelude::*;
use yew::services::ConsoleService;

mod websocket;

enum Msg {
    WebSocket(MsgWebSocket),
    User(MsgUser),
}
enum MsgWebSocket {
    Connect,
    Notify(websocket::Notify),
    ReceiveMessage(shared::ServerResponse),
    ReceiveError(anyhow::Error),
}
enum MsgUser {
    SendCommand(shared::Command),
    ClearErrors,
}
impl From<MsgWebSocket> for Msg {
    fn from(msg: MsgWebSocket) -> Self {
        Self::WebSocket(msg)
    }
}
impl From<MsgUser> for Msg {
    fn from(msg: MsgUser) -> Self {
        Self::User(msg)
    }
}
impl From<shared::Command> for Msg {
    fn from(cmd: shared::Command) -> Self {
        Self::User(MsgUser::SendCommand(cmd))
    }
}

struct Model {
    link: ComponentLink<Self>,
    websocket: websocket::Helper<shared::ClientRequest, shared::ServerResponse>,
    errors: Vec<String>,
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
                <Controls on_command=self.link.callback(|cmd| cmd) />
                <br/>
                { self.view_errors() }
            </div>
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
                ConsoleService::log("WEBSOCKET: Connecting...");
                match self.websocket.connect() {
                    Ok(_) => true,
                    Err(err) => {
                        self.push_error("websocket connect", err);
                        true
                    }
                }
            }
            MsgWebSocket::Notify(event) => {
                ConsoleService::info(&format!("WEBSOCKET: {:?}", event));
                self.websocket.on_notify(event)
            }
            MsgWebSocket::ReceiveMessage(message) => {
                ConsoleService::log(&format!("<- {:?}", message));
                match message {
                    shared::ServerResponse::Success => false,
                    shared::ServerResponse::ServerError(err_message) => {
                        self.push_error("server", err_message);
                        true
                    }
                }
            }
            MsgWebSocket::ReceiveError(err) => {
                ConsoleService::error(&format!("ERROR: {:?}", err));
                self.push_error("receive", err);
                true
            }
        }
    }
    fn update_user(&mut self, msg: MsgUser) -> ShouldRender {
        match msg {
            MsgUser::SendCommand(command) => {
                let payload = shared::ClientRequest::Command(command);
                ConsoleService::log(&format!("-> {:?}", &payload));
                if let Some(task) = self.websocket.get_task() {
                    task.send(&payload);
                }
                true
            }
            MsgUser::ClearErrors => {
                let was_empty = self.errors.is_empty();
                self.errors.clear();
                !was_empty
            }
        }
    }
}
fn create_websocket(
    link: &ComponentLink<Model>,
) -> websocket::Helper<shared::ClientRequest, shared::ServerResponse> {
    const URL_WEBSOCKET: &str = "ws://127.0.0.1:3030/ws";
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
        Self {
            link,
            websocket,
            errors: vec![],
        }
    }
    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            Msg::WebSocket(msg) => self.update_websocket(msg),
            Msg::User(msg) => self.update_user(msg),
        }
    }
    fn change(&mut self, _props: Self::Properties) -> ShouldRender {
        // no props
        false
    }
    fn view(&self) -> Html {
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
    }

    pub(crate) enum Msg {}

    pub(crate) struct Controls {
        on_command: Callback<Command>,
        link: ComponentLink<Self>,
    }
    impl Controls {
        fn view_buttons(&self) -> Html {
            let fetch_button = |text, cmd: Command| {
                html! {
                    <button onclick=self.on_command.reform(move |_| cmd.clone())>
                        { text }
                    </button>
                }
            };
            html! {
                <>
                    { fetch_button(SYMBOL_PREVIOUS, Command::SeekPrevious, ) }
                    { fetch_button(SYMBOL_PLAY, Command::PlaybackResume, ) }
                    { fetch_button(SYMBOL_PAUSE, Command::PlaybackPause, ) }
                    { fetch_button(SYMBOL_NEXT, Command::SeekNext, ) }
                </>
            }
        }
    }
    impl Component for Controls {
        type Message = Msg;
        type Properties = Properties;
        fn create(props: Self::Properties, link: ComponentLink<Self>) -> Self {
            let Properties { on_command } = props;
            Self { on_command, link }
        }
        fn update(&mut self, msg: Self::Message) -> ShouldRender {
            match msg {}
        }
        fn change(&mut self, props: Self::Properties) -> ShouldRender {
            let Properties { on_command } = props;
            self.on_command = on_command;
            // pessimistic
            true
        }
        fn view(&self) -> Html {
            html! {
                <div>
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
