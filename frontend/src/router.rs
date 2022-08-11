// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use yew::{html, Callback, Context, Html, Properties};
use yew_router::{
    prelude::{Link as RawLink, Redirect},
    Routable,
};

use crate::{log, model, view, websocket, App, AppMsgFull};

pub type Link = RawLink<Route>;
pub fn link_to_default() -> Html {
    html! {
        <Link to={Route::default()}>{"Back to Home"}</Link>
    }
}

#[derive(Clone, Routable, PartialEq)]
pub enum Route {
    #[at("/")]
    Root,
    #[at("/COPYING")]
    Copying,
    #[at("/debug")]
    DebugPanel,
    #[at("/player")]
    Player,
    #[not_found]
    #[at("/404")]
    NotFound,
}
impl Route {
    fn default() -> Self {
        Self::Player
    }
}

#[derive(Properties)]
pub(crate) struct Props {
    model: model::Data,
    on_message_opt: Callback<Option<AppMsgFull>>,
}
impl PartialEq for Props {
    fn eq(&self, other: &Self) -> bool {
        let Self {
            model,
            on_message_opt,
        } = self;
        *model == other.model && *on_message_opt == other.on_message_opt
    }
}

pub(crate) enum Main {}
impl Main {
    pub(crate) fn switch_elem(model: model::Data, ctx: &Context<App>) -> Html {
        let on_message_opt = ctx.link().batch_callback(|msg| msg);
        html! { <self::render_adapter::CustomSwitch<Self> {model} {on_message_opt} /> }
    }
}
impl self::render_adapter::Renderer for Main {
    type Route = Route;
    type Props = Props;

    fn render_view(
        route: &Self::Route,
        ctx: &Context<self::render_adapter::CustomSwitch<Self>>,
    ) -> Html {
        let Props {
            model,
            on_message_opt,
        } = ctx.props();
        //
        let on_message = on_message_opt.reform(Option::Some);
        let on_websocket = on_message.reform(AppMsgFull::from);
        let on_log = on_message.reform(AppMsgFull::from);
        let on_command = on_message.reform(AppMsgFull::from);
        let on_command_opt =
            on_message_opt.reform(|m: Option<shared::Command>| m.map(AppMsgFull::from));
        //
        match route {
            Route::Root => html! {
                <Redirect<Route> to={Route::default()} />
            },
            Route::Copying => html! { <view::Copying /> },
            Route::DebugPanel => {
                let websocket_connect = on_websocket.reform(|_| websocket::Msg::Connect);
                let websocket_disconnect = on_websocket.reform(|_| websocket::Msg::Disconnect);
                let fake_error =
                    on_log.reform(|_| log::Msg::Error(("debug", "fake error".to_string())));
                let fake_playpause = on_command.reform(|_| shared::Command::PlaybackPause);
                html! {
                <>
                    <div>
                        <button onclick={fake_error}>{ "Trigger fake error" }</button>
                        <button onclick={fake_playpause}>{ "PlayPause" }</button>
                    </div>
                    <div>
                        {"Websocket "}
                        <button onclick={websocket_connect}>{"Connect"}</button>
                        <button onclick={websocket_disconnect}>{"Disconnect"}</button>
                    </div>
                    <div>
                        <Link to={Route::Root}>{ "back to Home" }</Link>
                    </div>
                </>
                }
            }
            Route::Player => html! {
                <>
                    <h3>{"Player"}</h3>
                    <div class="row">
                        <div class="playback container col-5 col-s-7">
                            <view::Playback data={model.playback_status()} {on_command_opt} />
                        </div>
                        <div class="playback art col-7 col-s-5">
                            <view::AlbumArt data={model.playback_info()} />
                        </div>
                    </div>
                </>
            },
            Route::NotFound => html! {
                <>
                    <h3>{"Not Found :\\"}</h3>
                    { link_to_default() }
                </>
            },
        }
    }
}

mod render_adapter {
    use std::marker::PhantomData;
    use yew::{Component, Context, Html, Properties};
    use yew_router::{
        history::Location, prelude::RouterScopeExt, scope_ext::HistoryHandle, Routable,
    };

    pub trait Renderer {
        type Route: Routable + PartialEq;
        type Props: Properties + PartialEq + 'static;
        fn render_view(route: &Self::Route, ctx: &Context<CustomSwitch<Self>>) -> Html
        where
            Self: 'static;
    }

    pub enum Msg {
        ReRender,
    }
    pub struct CustomSwitch<T: ?Sized> {
        _listener: HistoryHandle,
        _phantom: PhantomData<T>,
    }
    impl<T> Component for CustomSwitch<T>
    where
        T: Renderer + 'static + ?Sized,
    {
        type Message = Msg;
        type Properties = <T as Renderer>::Props;

        fn create(ctx: &yew::Context<Self>) -> Self {
            let link = ctx.link();
            let listener = link
                .add_history_listener(link.callback(|_| Msg::ReRender))
                .expect("failed to create history handle. Do you have a router registered?");
            Self {
                _listener: listener,
                _phantom: PhantomData,
            }
        }

        fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
            match msg {
                Msg::ReRender => true,
            }
        }

        fn view(&self, ctx: &Context<Self>) -> Html {
            let route = ctx
                .link()
                .location()
                .and_then(|m| m.route::<<T as Renderer>::Route>());
            if let Some(ref route) = route {
                T::render_view(route, ctx)
            } else {
                Html::default()
            }
        }
    }
}
