use crate::fmt;
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
    pub received_time: shared::Time,
}
impl From<(&PlaybackStatus, &shared::Time)> for PositionInfo {
    fn from((playback, &received_time): (&PlaybackStatus, &shared::Time)) -> Self {
        Self {
            duration: playback.duration,
            position: playback.time, //TODO: ELIMINATE this confusing equality: "time" != "position" (!!!)
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
            let forecast_time = (shared::time_now() - self.received_time).num_seconds();
            let forecast_time = u64::try_from(forecast_time).unwrap_or(0);
            (self.position + forecast_time).min(self.duration)
        } else {
            // not playing, position unchanged
            self.position
        }
    }
}

pub(crate) enum Msg {
    PreviewPosition(u32),
}

pub(crate) struct PlaybackPosition {
    link: ComponentLink<Self>,
    // Callback to send `shared::Command`s
    on_command: Callback<Command>,
    // Duration of current item
    duration: u64,
    // Time status was received from server
    received_time: shared::Time,
    // Current forecast position
    forecast_position: u64,
    // Preview slider position (while sliding)
    preview_position: Option<u64>,
}
impl Component for PlaybackPosition {
    type Properties = Properties;
    type Message = Msg;
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
            received_time: position_info.received_time,
            forecast_position,
            preview_position: None,
        }
    }
    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            Msg::PreviewPosition(position) => {
                self.preview_position = Some(u64::from(position));
                true
            }
        }
    }
    fn change(&mut self, props: Self::Properties) -> ShouldRender {
        let Properties {
            on_command,
            position_info,
        } = props;
        self.on_command = on_command; // Callback's `PartialEq` implementation is empirically useless
        if position_info.received_time != self.received_time {
            self.preview_position = None;
        }
        let forecast_position = position_info.calc_forecast_position();
        let duration = position_info.duration;
        set_detect_change! {
            self.duration = duration;
            self.forecast_position = forecast_position;
            self.received_time = position_info.received_time;
        }
    }
    fn view(&self) -> Html {
        log_render!("PlaybackPosition");
        let duration = self.duration;
        let position = self.preview_position.unwrap_or(self.forecast_position);
        let remaining = duration.saturating_sub(position);
        let duration_str = duration.to_string();
        let position_str = position.to_string();
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
        let position_fmt = fmt::fmt_duration_seconds(position);
        let remaining_fmt = fmt::fmt_duration_seconds(remaining);
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
