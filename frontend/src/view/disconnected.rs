// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use crate::{colors, fmt, svg, App};
use std::borrow::Cow;
use yew::{html, html::IntoPropValue, Callback, Children, Properties};

#[derive(PartialEq)]
pub struct Data {
    pub is_websocket_started: bool,
    pub is_after_first_reconnect: bool,
    pub reconnect_timeout_millis: Option<u32>,
    reconnect_is_connected: bool,
}
#[derive(Properties, PartialEq)]
pub struct Props {
    pub data: Data,
    #[prop_or_default]
    pub children: Children,
    pub on_reconnect_now: Callback<()>,
}
impl IntoPropValue<Data> for &App {
    fn into_prop_value(self) -> Data {
        Data {
            is_websocket_started: self.websocket.is_connection_started(),
            is_after_first_reconnect: self.reconnect.is_after_first_reconnect(),
            reconnect_timeout_millis: self.reconnect.get_timeout_millis(),
            reconnect_is_connected: self.reconnect.is_connected(),
        }
    }
}
impl Data {
    fn is_connected(&self) -> bool {
        self.is_websocket_started && self.reconnect_is_connected
    }
}

#[yew::function_component(Disconnected)]
pub fn disconnected(props: &Props) -> Html {
    const RED_FILL: svg::Renderer = svg::Renderer {
        stroke: colors::NONE,
        fill: colors::RED,
    };
    let reconnect_msg = if props.data.is_websocket_started {
        "Connecting...".into()
    } else {
        let auto_reconnect_status: Cow<'static, str> = props.data.reconnect_timeout_millis.map_or(
            "Auto-reconnect has given up (not scheduled).".into(),
            |millis| {
                let seconds = f64::from(millis) / 1000.0;
                #[allow(clippy::cast_possible_truncation)]
                #[allow(clippy::cast_sign_loss)]
                let seconds = seconds.abs().trunc() as u64;
                let seconds = fmt::fmt_duration_seconds_long(seconds);
                format!("Trying to reconnect in {seconds}").into()
            },
        );

        let onclick = props.on_reconnect_now.reform(|_| {});
        html! {
            <div>
                <span>{ auto_reconnect_status }</span>
                <span>
                    <button {onclick}>
                        { "Reconnect Now" }
                    </button>
                </span>
            </div>
        }
    };
    html! {
        if props.data.is_connected() {
            { for props.children.iter() }
        } else {
            <div class={"row"}>
                if props.data.is_after_first_reconnect {
                    <div class={"disconnected col-s-2 keep-true-color"}>
                        { RED_FILL.render(svg::X_CROSS) }
                    </div>
                }
                <div class="disconnected col-s-10">
                    <span class="title">{ "Connecting to server..." }</span>
                    { reconnect_msg }
                </div>
            </div>
        }
    }
}
