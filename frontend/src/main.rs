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
            </div>
        }
    }
}

fn main() {
    yew::start_app::<Model>();
}
