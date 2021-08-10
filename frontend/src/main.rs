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
use yew::services::fetch::{FetchService, FetchTask, Request, Response};

#[derive(Debug)]
pub(crate) enum ApiFetch {
    Basic { uri: String },
}

enum Msg {
    Fetch(ApiFetch),
    ReceiveSuccessResponse(shared::Success),
    ReceiveError(anyhow::Error),
}
impl From<ApiFetch> for Msg {
    fn from(other: ApiFetch) -> Self {
        Self::Fetch(other)
    }
}

struct Model {
    link: ComponentLink<Self>,
    active_fetch: Option<FetchTask>,
    queued_fetch: Vec<ApiFetch>,
    errors: Vec<anyhow::Error>,
}
impl Model {
    fn start_fetch(&mut self, fetch: ApiFetch) {
        use yew::format::{Json, Nothing};
        //
        let request = match fetch {
            ApiFetch::Basic { uri } => Request::get(uri)
                .body(Nothing)
                .expect("Could not build request."),
        };
        let callback = self.link.callback(
            |response: Response<Json<Result<shared::Success, anyhow::Error>>>| {
                let Json(data) = response.into_body();
                match data {
                    Ok(response) => Msg::ReceiveSuccessResponse(response),
                    Err(err) => Msg::ReceiveError(err),
                }
            },
        );
        let task = FetchService::fetch(request, callback).expect("failed to start request");
        self.active_fetch.replace(task);
    }
}
impl Component for Model {
    type Message = Msg;
    type Properties = ();
    fn create(_props: Self::Properties, link: ComponentLink<Self>) -> Self {
        Self {
            link,
            active_fetch: None,
            queued_fetch: vec![],
            errors: vec![],
        }
    }
    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            Msg::Fetch(fetch) => {
                self.queued_fetch.push(fetch);
            }
            Msg::ReceiveSuccessResponse(data) => {
                self.active_fetch.take();
            }
            Msg::ReceiveError(err) => {
                self.active_fetch.take();
                self.errors.push(err);
            }
        }
        if self.active_fetch.is_none() {
            if let Some(fetch) = self.queued_fetch.pop() {
                self.start_fetch(fetch);
            }
        }
        true
    }
    fn change(&mut self, _props: Self::Properties) -> ShouldRender {
        // no props
        false
    }
    fn view(&self) -> Html {
        html! {
            <div>
                <p>{ "This is generated in Yew!" }</p>
                <NumFetcher />
                <Controls on_fetch=self.link.callback(|api| api) />
                <p>{ format!("running: {:?}", &self.active_fetch) }</p>
                <p>{ format!("queue: {:?}", &self.queued_fetch) }</p>
                <p>{ format!("errors: {:?}", &self.errors) }</p>
            </div>
        }
    }
}
use controls::Controls;
mod controls {
    use crate::ApiFetch;
    use yew::prelude::*;

    // reference table: https://stackoverflow.com/a/27053825/5742216
    const SYMBOL_PREVIOUS: &str = "\u{23EE}";
    const SYMBOL_NEXT: &str = "\u{23ED}";
    const SYMBOL_PLAY: &str = "\u{23F5}";
    const SYMBOL_PAUSE: &str = "\u{23F8}";

    #[derive(Properties, Clone)]
    pub(crate) struct Properties {
        pub on_fetch: Callback<ApiFetch>,
    }

    pub(crate) enum Msg {}

    pub(crate) struct Controls {
        on_fetch: Callback<ApiFetch>,
        link: ComponentLink<Self>,
    }
    impl Controls {
        fn view_buttons(&self) -> Html {
            let fetch_button = |uri: &'static str, text| {
                html! {
                    <button onclick=self.on_fetch.reform(move |_| ApiFetch::Basic {
                        uri: uri.to_string(),
                    })>
                        { text }
                    </button>
                }
            };
            html! {
                <>
                    { fetch_button("/v1/previous", SYMBOL_PREVIOUS) }
                    { fetch_button("/v1/play", SYMBOL_PLAY) }
                    { fetch_button("/v1/pause", SYMBOL_PAUSE) }
                    { fetch_button("/v1/next", SYMBOL_NEXT) }
                </>
            }
        }
    }
    impl Component for Controls {
        type Message = Msg;
        type Properties = Properties;
        fn create(props: Self::Properties, link: ComponentLink<Self>) -> Self {
            let Properties { on_fetch } = props;
            Self { on_fetch, link }
        }
        fn update(&mut self, msg: Self::Message) -> ShouldRender {
            match msg {}
        }
        fn change(&mut self, props: Self::Properties) -> ShouldRender {
            let Properties { on_fetch } = props;
            self.on_fetch = on_fetch;
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
