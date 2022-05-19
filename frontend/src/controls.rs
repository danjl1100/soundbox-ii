// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use crate::svg;
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

#[derive(Properties, Clone)]
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

pub(crate) struct Controls {
    on_command: Callback<Command>,
    link: ComponentLink<Self>,
    playback_state: Option<shared::PlaybackState>,
    ty: Type,
}
impl Controls {
    fn view_buttons(&self) -> Html {
        const SEEK_BACKWARD: shared::Command = Command::SeekRelative { seconds_delta: -5 };
        const SEEK_FORWARD: shared::Command = Command::SeekRelative { seconds_delta: 5 };
        const VOL_DOWN: shared::Command = Command::VolumeRelative { percent_delta: -5 };
        const VOL_UP: shared::Command = Command::VolumeRelative { percent_delta: 5 };
        let is_paused = self.playback_state == Some(shared::PlaybackState::Paused);
        let is_playing = self.playback_state == Some(shared::PlaybackState::Playing);
        match self.ty {
            Type::TrackPause => html! {
                <>
                    { self.fetch_button(LABEL_PREVIOUS, Command::SeekPrevious, true) }
                    { self.fetch_button(LABEL_PLAY, Command::PlaybackResume, !is_playing) }
                    { self.fetch_button(LABEL_PAUSE, Command::PlaybackPause, !is_paused) }
                    { self.fetch_button(LABEL_NEXT, Command::SeekNext, true) }
                </>
            },
            Type::Seek => html! {
                <>
                    { self.fetch_button(LABEL_BACKWARD, SEEK_BACKWARD, true) }
                    { self.fetch_button(LABEL_FORWARD, SEEK_FORWARD, true) }
                </>
            },
            Type::Volume => html! {
                <>
                    { self.fetch_button(LABEL_SOFTER, VOL_DOWN, true) }
                    { self.fetch_button(LABEL_LOUDER, VOL_UP, true) }
                </>
            },
        }
    }
    fn fetch_button(&self, (text, svg_def): (&str, &svg::Def), cmd: Command, enable: bool) -> Html {
        const BLACK: svg::Renderer = svg::Renderer {
            stroke: "none",
            fill: "black",
        };
        let style = if enable { "" } else { "display: none;" };
        html! {
            <button onclick=self.on_command.reform(move |_| cmd.clone()) style=style>
                { BLACK.render(svg_def) }
                { text }
            </button>
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
            ty,
        } = props;
        Self {
            on_command,
            link,
            playback_state,
            ty,
        }
    }
    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {}
    }
    fn change(&mut self, props: Self::Properties) -> ShouldRender {
        let Properties {
            on_command,
            playback_state,
            ty,
        } = props;
        self.on_command = on_command; // Callback's `PartialEq` implementation is empirically useless
        set_detect_change! {
            self.ty = ty;
            self.playback_state = playback_state;
        }
    }
    fn view(&self) -> Html {
        log_render!(format!("Controls {:?}", self.ty));
        self.view_buttons()
    }
}
