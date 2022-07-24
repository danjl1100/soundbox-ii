// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use super::svg;
use shared::Command;
use yew::prelude::*;

const LABEL_PREVIOUS: (&str, &svg::Def) = ("Previous", svg::PREV);
const LABEL_NEXT: (&str, &svg::Def) = ("Next", svg::NEXT);
const LABEL_PLAY: (&str, &svg::Def) = ("Play", svg::PLAY);
const LABEL_PAUSE: (&str, &svg::Def) = ("Pause", svg::PAUSE);
const LABEL_FORWARD: (&str, &svg::Def) = ("Forward", svg::FORWARD);
const LABEL_BACKWARD: (&str, &svg::Def) = ("Backward", svg::BACKWARD);
const LABEL_LOUDER: (&str, &svg::Def) = ("Louder", svg::PLUS);
const LABEL_SOFTER: (&str, &svg::Def) = ("Softer", svg::MINUS);

#[derive(Properties, Clone, PartialEq)]
pub(crate) struct Properties {
    pub on_command: Callback<Command>,
    pub playback_state: Option<shared::PlaybackState>,
    pub ty: Type,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Type {
    TrackPause,
    Seek,
    Volume,
}

pub(crate) enum Msg {}

// TODO - make this a functional component (with fn-inside-fn 'ception)
pub(crate) struct Controls;
impl Controls {
    fn view_buttons(ctx: &Context<Self>) -> Html {
        const SEEK_BACKWARD: shared::Command = Command::SeekRelative { seconds_delta: -5 };
        const SEEK_FORWARD: shared::Command = Command::SeekRelative { seconds_delta: 5 };
        const VOL_DOWN: shared::Command = Command::VolumeRelative { percent_delta: -5 };
        const VOL_UP: shared::Command = Command::VolumeRelative { percent_delta: 5 };
        let props = ctx.props();
        let is_paused = props.playback_state == Some(shared::PlaybackState::Paused);
        let is_playing = props.playback_state == Some(shared::PlaybackState::Playing);
        match props.ty {
            Type::TrackPause => html! {
                <>
                    { Self::fetch_button(ctx, LABEL_PREVIOUS, Command::SeekPrevious, true) }
                    { Self::fetch_button(ctx, LABEL_PLAY, Command::PlaybackResume, !is_playing) }
                    { Self::fetch_button(ctx, LABEL_PAUSE, Command::PlaybackPause, !is_paused) }
                    { Self::fetch_button(ctx, LABEL_NEXT, Command::SeekNext, true) }
                </>
            },
            Type::Seek => html! {
                <>
                    { Self::fetch_button(ctx, LABEL_BACKWARD, SEEK_BACKWARD, true) }
                    { Self::fetch_button(ctx, LABEL_FORWARD, SEEK_FORWARD, true) }
                </>
            },
            Type::Volume => html! {
                <>
                    { Self::fetch_button(ctx, LABEL_SOFTER, VOL_DOWN, true) }
                    { Self::fetch_button(ctx, LABEL_LOUDER, VOL_UP, true) }
                </>
            },
        }
    }
    fn fetch_button(
        ctx: &Context<Self>,
        (text, svg_def): (&str, &svg::Def),
        cmd: Command,
        enable: bool,
    ) -> Html {
        const BLACK: svg::Renderer = svg::Renderer {
            stroke: "none",
            fill: "black",
        };
        let props = ctx.props();
        let style = if enable { "" } else { "display: none;" };
        let onclick = props.on_command.reform(move |_| cmd.clone());
        html! {
            <button {onclick} {style}>
                { BLACK.render(svg_def) }
                { text }
            </button>
        }
    }
}
impl Component for Controls {
    type Message = Msg;
    type Properties = Properties;
    fn create(_ctx: &Context<Self>) -> Self {
        Self
    }
    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {}
    }
    // fn change(&mut self, ctx: &Context<Self>, props: Self::Properties) -> ShouldRender {
    //     let Properties {
    //         on_command,
    //         playback_state,
    //         ty,
    //     } = props;
    //     self.on_command = on_command; // Callback's `PartialEq` implementation is empirically useless
    //     set_detect_change! {
    //         self.ty = ty;
    //         self.playback_state = playback_state;
    //     }
    // }
    fn view(&self, ctx: &Context<Self>) -> Html {
        log_render!(format!("Controls {:?}", ctx.props().ty));
        Self::view_buttons(ctx)
    }
}
