// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use crate::fmt;
use web_sys::HtmlInputElement;
use yew::{function_component, html, Callback, Component, Context, Html, NodeRef, Properties};

#[derive(Properties, PartialEq)]
pub struct Props {
    pub playback_timing: shared::PlaybackTiming,
    pub received_time: shared::Time,
    pub on_command_opt: Callback<Option<shared::Command>>,
}
#[derive(Debug)]
pub enum Msg {
    PreviewSeekInput { seconds: u32 },
}
pub struct PlaybackPosition {
    /// User-input value (for responsive UI before server acknowledge)
    preview_position_secs: Option<u64>,
    input_ref: NodeRef,
}
impl Component for PlaybackPosition {
    type Message = Msg;
    type Properties = Props;

    fn create(_ctx: &yew::Context<Self>) -> Self {
        Self {
            preview_position_secs: None,
            input_ref: NodeRef::default(),
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::PreviewSeekInput { seconds } => {
                self.preview_position_secs = Some(u64::from(seconds));
                true
            }
        }
    }

    fn changed(&mut self, _ctx: &Context<Self>) -> bool {
        self.preview_position_secs = None;
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let Props {
            on_command_opt,
            playback_timing,
            received_time,
        } = ctx.props();
        let on_seek = {
            let input_ref = self.input_ref.clone();
            on_command_opt.reform(move |_| {
                let seconds = parse_position_str(&input_ref)?;
                Some(shared::Command::SeekTo { seconds })
            })
        };
        let on_seek_preview = {
            let input_ref = self.input_ref.clone();
            ctx.link().batch_callback(move |_| {
                let seconds = parse_position_str(&input_ref)?;
                Some(Msg::PreviewSeekInput { seconds })
            })
        };
        let Self {
            preview_position_secs,
            input_ref: slider_input_ref,
        } = self;
        html! {
            <InnerPlaybackPosition
                playback_timing={*playback_timing}
                preview_position_secs={*preview_position_secs}
                received_time={*received_time}
                {on_seek}
                {on_seek_preview}
                {slider_input_ref}
                />
        }
    }
}
fn parse_position_str(input_elem: &NodeRef) -> Option<u32> {
    input_elem
        .cast::<HtmlInputElement>()
        .map(|elem| elem.value())
        .and_then(|value| {
            use std::str::FromStr;
            u32::from_str(&value).ok()
        })
}

#[derive(Properties, PartialEq)]
struct InnerProps {
    playback_timing: shared::PlaybackTiming,
    received_time: shared::Time,
    preview_position_secs: Option<u64>,
    on_seek: Callback<web_sys::Event>,
    on_seek_preview: Callback<web_sys::InputEvent>,
    slider_input_ref: NodeRef,
}
impl InnerProps {
    fn calc_forecast_position_secs(&self) -> u64 {
        let shared::PlaybackTiming { position_secs, .. } = self
            .playback_timing
            .predict_change(shared::time_now() - self.received_time);
        position_secs
    }
}
#[function_component(InnerPlaybackPosition)]
fn inner_playback_position(props: &InnerProps) -> Html {
    const LABEL_TIME_NONE: &str = "--:--";
    struct Info {
        duration_str: String,
        position_str: String,
        position_fmt: String,
        remaining_fmt: String,
    }
    let InnerProps {
        playback_timing: shared::PlaybackTiming { duration_secs, .. },
        received_time: _, // used in forecast
        preview_position_secs,
        on_seek,
        on_seek_preview,
        slider_input_ref,
    } = props;
    let has_duration = *duration_secs > 0;
    let info = has_duration.then_some(()).map(|()| {
        let position_secs =
            preview_position_secs.unwrap_or_else(|| props.calc_forecast_position_secs());
        let remaining_secs = duration_secs.saturating_sub(position_secs);
        Info {
            duration_str: format!("{duration_secs}"),
            position_str: format!("{position_secs}"),
            position_fmt: fmt::fmt_duration_seconds(position_secs),
            remaining_fmt: fmt::fmt_duration_seconds(remaining_secs),
        }
    });
    if info.is_some() {
        // `yew_hooks` shenanigans to trigger re-render every second
        let dummy_state = yew::use_state(|| ());
        let touch_state = move || {
            dummy_state.set(());
        };
        yew_hooks::use_interval(touch_state, 1000);
    }
    html! {
        <div class="playback time">
            if let Some(info) = info {
                { info.position_fmt }
                <input type="range"
                    min="0" max={info.duration_str} value={info.position_str}
                    ref={slider_input_ref.clone()}
                    onchange={on_seek}
                    oninput={on_seek_preview}
                    />
                { "-" }{ info.remaining_fmt }
            } else {
                { LABEL_TIME_NONE }
                <input type="range" min="0" max="0" class="disabled" />
                { LABEL_TIME_NONE }
            }
        </div>
    }
}
