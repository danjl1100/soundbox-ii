// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use crate::fmt;
use shared::{Command, PlaybackInfo, PlaybackTiming};
use yew::prelude::*;

#[derive(Properties, Clone)]
pub(crate) struct Properties {
    pub on_command: Callback<Command>,
    pub timing: PlaybackTiming,
    pub received_time: shared::Time,
}

pub(crate) enum Msg {
    PreviewPosition(u32),
}

pub(crate) struct PlaybackPosition {
    link: ComponentLink<Self>,
    // Callback to send `shared::Command`s
    on_command: Callback<Command>,
    // Duration of current item
    duration_secs: u64,
    // Time status was received from server
    received_time: shared::Time,
    // Current forecast position
    forecast_position_secs: u64,
    // Preview slider position (while sliding)
    preview_position_secs: Option<u64>,
}
impl Properties {
    fn calc_forecast_position_secs(&self) -> u64 {
        let PlaybackTiming { position_secs, .. } = self
            .timing
            .predict_change(shared::time_now() - self.received_time);
        position_secs
    }
}
impl Component for PlaybackPosition {
    type Properties = Properties;
    type Message = Msg;
    fn create(props: Self::Properties, link: ComponentLink<Self>) -> Self {
        let forecast_position_secs = props.calc_forecast_position_secs();
        let Properties {
            on_command,
            timing: PlaybackTiming { duration_secs, .. },
            received_time,
        } = props;
        Self {
            link,
            on_command,
            duration_secs,
            received_time,
            forecast_position_secs,
            preview_position_secs: None,
        }
    }
    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            Msg::PreviewPosition(position_secs) => {
                self.preview_position_secs = Some(u64::from(position_secs));
                true
            }
        }
    }
    fn change(&mut self, props: Self::Properties) -> ShouldRender {
        let forecast_position_secs = props.calc_forecast_position_secs();
        let Properties {
            on_command,
            timing: PlaybackTiming { duration_secs, .. },
            received_time: new_received_time,
        } = props;
        self.on_command = on_command; // Callback's `PartialEq` implementation is empirically useless
        if new_received_time != self.received_time {
            self.preview_position_secs = None;
        }
        set_detect_change! {
            self.duration_secs = duration_secs;
            self.forecast_position_secs = forecast_position_secs;
            self.received_time = new_received_time;
        }
    }
    fn view(&self) -> Html {
        log_render!("PlaybackPosition");
        let duration_secs = self.duration_secs;
        let position_secs = self
            .preview_position_secs
            .unwrap_or(self.forecast_position_secs);
        let remaining_secs = duration_secs.saturating_sub(position_secs);
        let duration_str = duration_secs.to_string();
        let position_str = position_secs.to_string();
        let on_change = self.on_command.reform(|change| match change {
            ChangeData::Value(s) => {
                let seconds = parse_position_str(&s);
                shared::Command::SeekTo { seconds }
            }
            _ => unreachable!("range input gives Value"),
        });
        let on_input = self
            .link
            .callback(|event: InputData| Msg::PreviewPosition(parse_position_str(&event.value)));
        let position_fmt = fmt::fmt_duration_seconds(position_secs);
        let remaining_fmt = fmt::fmt_duration_seconds(remaining_secs);
        html! {
            <div class="playback time">
                { position_fmt }
                <input type="range"
                    min="0" max=duration_str value=position_str
                    onchange=on_change
                    oninput=on_input
                    />
                { "-" }{ remaining_fmt }
            </div>
        }
    }
}
fn parse_position_str(seconds: &str) -> u32 {
    use std::str::FromStr;
    u32::from_str(seconds).expect("range input gives integer value")
}

pub(crate) enum PlaybackMeta {}
impl PlaybackMeta {
    pub fn render(info: &PlaybackInfo) -> Html {
        let artist = if info.artist.is_empty() {
            "[No Artist]"
        } else {
            &info.artist
        };
        let album = if info.album.is_empty() {
            "[No Album]"
        } else {
            &info.album
        };
        html! {
            <>
                <div>
                    <span class="title">{ &info.title }</span>
                </div>
                <div>
                    <span>
                        <span class="artist">{ artist }</span>
                        { " \u{2014} " }
                        <span class="album">{ album }</span>
                    </span>
                </div>
            </>
        }
    }
}
