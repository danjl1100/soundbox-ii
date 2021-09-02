use crate::{fmt, Time};
use shared::{Command, PlaybackInfo, PlaybackStatus};
use yew::prelude::*;

#[derive(Properties, Clone)]
pub(crate) struct Properties {
    pub on_command: Callback<Command>,
    pub position_info: PositionInfo,
}
#[derive(Clone)]
pub(crate) struct PositionInfo {
    pub duration: u64,
    pub position: u64,
    pub playback_state: shared::PlaybackState,
    pub received_time: Time,
}
impl From<(&PlaybackStatus, &Time)> for PositionInfo {
    fn from((playback, &received_time): (&PlaybackStatus, &Time)) -> Self {
        Self {
            duration: playback.duration,
            position: playback.time,
            playback_state: playback.state,
            received_time,
        }
    }
}
impl PositionInfo {
    fn calc_forecast_position(&self) -> u64 {
        use std::convert::TryFrom;
        if self.playback_state == shared::PlaybackState::Playing {
            // playing, forecast current position
            let forecast_time = (chrono::Utc::now() - self.received_time).num_seconds();
            let forecast_time = u64::try_from(forecast_time).unwrap_or(0);
            (self.position + forecast_time).min(self.duration)
        } else {
            // not playing, position unchanged
            self.position
        }
    }
}

pub(crate) struct PlaybackPosition {
    link: ComponentLink<Self>,
    on_command: Callback<Command>,
    duration: u64,
    forecast_position: u64,
}
impl Component for PlaybackPosition {
    type Properties = Properties;
    type Message = ();
    fn create(props: Self::Properties, link: ComponentLink<Self>) -> Self {
        let Properties {
            on_command,
            position_info,
        } = props;
        let forecast_position = position_info.calc_forecast_position();
        let duration = position_info.duration;
        Self {
            link,
            on_command,
            duration,
            forecast_position,
        }
    }
    fn update(&mut self, _: Self::Message) -> ShouldRender {
        false
    }
    fn change(&mut self, props: Self::Properties) -> ShouldRender {
        let Properties {
            on_command,
            position_info,
        } = props;
        self.on_command = on_command; // Callback's `PartialEq` implementation is empirically useless
        let forecast_position = position_info.calc_forecast_position();
        let duration = position_info.duration;
        set_detect_change! {
            self.duration = duration;
            self.forecast_position = forecast_position;
        }
    }
    fn view(&self) -> Html {
        use std::str::FromStr;
        log_render!("PlaybackPosition");
        let duration = self.duration;
        let position = self.forecast_position;
        let remaining = duration.saturating_sub(position);
        let duration_str = duration.to_string();
        let position_str = position.to_string();
        let on_change = self.on_command.reform(|change| match change {
            ChangeData::Value(s) => {
                let seconds = u32::from_str(&s).expect("range input gives integer value");
                shared::Command::SeekTo { seconds }
            }
            _ => unreachable!("range input gives Value"),
        });
        let position_fmt = fmt::fmt_duration_seconds(position);
        let remaining_fmt = fmt::fmt_duration_seconds(remaining);
        html! {
            <div class="playback time">
                { position_fmt }
                <input type="range"
                    min="0" max=duration_str value=position_str
                    onchange=on_change
                    />
                { "-" }{ remaining_fmt }
            </div>
        }
    }
}

pub(crate) enum PlaybackMeta {}
impl PlaybackMeta {
    pub fn render(info: &PlaybackInfo) -> Html {
        html! {
            <div class="playback meta">
                <p>{ &info.title }</p>
                <p>{ &info.artist }{ " - " }{ &info.album }</p>
            </div>
        }
    }
}
