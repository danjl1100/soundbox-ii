use yew::prelude::*;

struct Model {
    link: ComponentLink<Self>,
}
impl Component for Model {
    type Message = ();
    type Properties = ();
    fn create(_props: Self::Properties, link: ComponentLink<Self>) -> Self {
        Self { link }
    }
    fn update(&mut self, _msg: Self::Message) -> ShouldRender {
        // no message
        false
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
            </div>
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
