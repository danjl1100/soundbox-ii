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
    WebsocketConnect,
    SendCommand(shared::Command),
    ReceiveMessage(shared::ServerResponse),
    ReceiveError(anyhow::Error),
    WebsocketNotify(websocket::Notify),
}
impl From<shared::Command> for Msg {
    fn from(cmd: shared::Command) -> Self {
        Self::SendCommand(cmd)
    }
}

struct Model {
    link: ComponentLink<Self>,
    websocket: websocket::Helper<shared::ClientRequest, shared::ServerResponse>,
    errors: Vec<anyhow::Error>,
}
impl Model {
    fn view_disconnected(&self) -> Html {
        html! {
            <div>
                { "Disconnected from server, that's sad :/" }
                <br/>
                <button onclick=self.link.callback(|_| Msg::WebsocketConnect)>
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
                <p>{ format!("errors: {:?}", &self.errors) }</p>
            </div>
        }
    }
}
impl Component for Model {
    type Message = Msg;
    type Properties = ();
    fn create(_props: Self::Properties, link: ComponentLink<Self>) -> Self {
        let websocket = {
            const URL_WEBSOCKET: &str = "ws://127.0.0.1:3030/ws";
            let on_message = link.callback(|msg| match msg {
                Ok(msg) => Msg::ReceiveMessage(msg),
                Err(e) => Msg::ReceiveError(e),
            });
            let on_notification = link.callback(Msg::WebsocketNotify);
            websocket::Helper::new(URL_WEBSOCKET, &on_message, on_notification)
        };
        link.send_message(Msg::WebsocketConnect);
        Self {
            link,
            websocket,
            errors: vec![],
        }
    }
    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            Msg::WebsocketConnect => {
                ConsoleService::log("WEBSOCKET: Connecting...");
                match self.websocket.connect() {
                    Ok(_) => true,
                    Err(err) => {
                        self.errors.push(err.into());
                        true
                    }
                }
            }
            Msg::SendCommand(command) => {
                let payload = shared::ClientRequest::Command(command);
                ConsoleService::log(&format!("-> {:?}", &payload));
                if let Some(task) = self.websocket.get_task() {
                    task.send(&payload);
                }
                true
            }
            Msg::ReceiveMessage(message) => {
                ConsoleService::log(&format!("<- {:?}", message));
                false
            }
            Msg::ReceiveError(err) => {
                ConsoleService::error(&format!("ERROR: {:?}", err));
                self.errors.push(err);
                true
            }
            Msg::WebsocketNotify(event) => {
                ConsoleService::info(&format!("WEBSOCKET: {:?}", event));
                self.websocket.on_notify(event)
            }
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
