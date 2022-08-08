// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use crate::{colors, svg};
use shared::Command;
use yew::{function_component, html, Callback, Html, Properties};

const LABEL_PREVIOUS: (&str, &svg::Def) = ("Previous", svg::PREV);
const LABEL_NEXT: (&str, &svg::Def) = ("Next", svg::NEXT);
const LABEL_PLAY: (&str, &svg::Def) = ("Play", svg::PLAY);
const LABEL_PAUSE: (&str, &svg::Def) = ("Pause", svg::PAUSE);
const LABEL_FORWARD: (&str, &svg::Def) = ("Forward", svg::FORWARD);
const LABEL_BACKWARD: (&str, &svg::Def) = ("Backward", svg::BACKWARD);
const LABEL_LOUDER: (&str, &svg::Def) = ("Louder", svg::PLUS);
const LABEL_SOFTER: (&str, &svg::Def) = ("Softer", svg::MINUS);

#[derive(Properties, PartialEq)]
pub struct Props {
    pub on_command: Callback<shared::Command>,
    pub playback_timing_state: shared::PlaybackState,
}
#[derive(Properties, PartialEq)]
pub struct CmdProps {
    pub on_command: Callback<shared::Command>,
}

#[function_component(TrackPause)]
pub fn track_pause(props: &Props) -> Html {
    let Props {
        on_command: cb,
        playback_timing_state,
    } = props;
    let is_paused = *playback_timing_state == shared::PlaybackState::Paused;
    let is_playing = *playback_timing_state == shared::PlaybackState::Playing;
    html! {
        <>
            { button(cb, LABEL_PREVIOUS, Command::SeekPrevious) }
            if !is_playing {
                { button(cb, LABEL_PLAY, Command::PlaybackResume) }
            }
            if !is_paused {
                { button(cb, LABEL_PAUSE, Command::PlaybackPause) }
            }
            { button(cb, LABEL_NEXT, Command::SeekNext) }
        </>
    }
}

#[function_component(Seek)]
pub fn seek(CmdProps { on_command: cb }: &CmdProps) -> Html {
    const SEEK_BACKWARD: shared::Command = Command::SeekRelative { seconds_delta: -5 };
    const SEEK_FORWARD: shared::Command = Command::SeekRelative { seconds_delta: 5 };
    html! {
        <>
            { button(cb, LABEL_BACKWARD, SEEK_BACKWARD) }
            { button(cb, LABEL_FORWARD, SEEK_FORWARD) }
        </>
    }
}

#[function_component(Volume)]
pub fn volume(CmdProps { on_command: cb }: &CmdProps) -> Html {
    const VOL_DOWN: shared::Command = Command::VolumeRelative { percent_delta: -5 };
    const VOL_UP: shared::Command = Command::VolumeRelative { percent_delta: 5 };
    html! {
        <>
            { button(cb, LABEL_SOFTER, VOL_DOWN) }
            { button(cb, LABEL_LOUDER, VOL_UP) }
        </>
    }
}

fn button(
    on_command: &Callback<Command>,
    (text, svg_def): (&str, &svg::Def),
    cmd: Command,
) -> Html {
    const BLACK_FILL: svg::Renderer = svg::Renderer {
        stroke: colors::NONE,
        fill: colors::BLACK,
    };
    let onclick = on_command.reform(move |_| cmd.clone());
    html! {
        <button {onclick}>
            { BLACK_FILL.render(svg_def) }
            { text }
        </button>
    }
}
