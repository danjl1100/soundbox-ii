// Copyright (C) 2021-2023  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use std::rc::Rc;
use yew::{function_component, html, html::IntoPropValue, Callback, Properties};

mod controls;
mod timing;

#[derive(PartialEq)]
struct DataInner {
    playback_status: Rc<shared::PlaybackStatus>,
    received_time: shared::Time,
}
#[derive(PartialEq)]
pub struct Data(Option<DataInner>);
#[derive(Properties, PartialEq)]
pub struct Props {
    pub data: Data,
    pub on_command_opt: Callback<Option<shared::Command>>,
}
impl IntoPropValue<Data> for Option<&(shared::PlaybackStatus, shared::Time)> {
    fn into_prop_value(self) -> Data {
        Data(self.map(|(playback, received_time)| DataInner {
            received_time: *received_time,
            playback_status: Rc::new(playback.clone()),
        }))
    }
}

#[function_component(Playback)]
pub fn playback(props: &Props) -> Html {
    if let Data(Some(data)) = &props.data {
        let on_command_opt = &props.on_command_opt;
        let on_command = on_command_opt.reform(Option::Some);
        let DataInner {
            playback_status,
            received_time,
        } = data;
        let playback_timing = playback_status.timing;
        let playback_timing_state = playback_status.timing.state;
        let volume_str = format!("{}%", playback_status.volume_percent);
        html! {
            <>
                <div class="playback control">
                    <controls::TrackPause
                        on_command={on_command.clone()}
                        {playback_timing_state}
                        />
                </div>
                <div class="playback meta">
                    <info::PlaybackMeta {playback_status} />
                    <timing::PlaybackPosition
                        {playback_timing}
                        received_time={*received_time}
                        {on_command_opt}
                        />
                    <div class="playback control">
                        <span>
                            <label>{ "Seek" }</label>
                            <controls::Seek on_command={on_command.clone()} />
                        </span>
                        <span>
                            <label>{ "Volume" }</label>
                            <controls::Volume {on_command} />
                            <label>{ volume_str }</label>
                        </span>
                    </div>
                </div>
            </>
        }
    } else {
        html! { "No playback status... yet." }
    }
}

mod info {
    use std::rc::Rc;
    use yew::{function_component, html, Properties};

    #[derive(Properties, PartialEq)]
    pub struct Props {
        pub playback_status: Rc<shared::PlaybackStatus>,
    }
    #[function_component(PlaybackMeta)]
    pub fn playback_meta(props: &Props) -> Html {
        const SEPARATOR: &str = " \u{2014} ";
        struct Info<'a> {
            title: &'a str,
            artist: &'a str,
            album: &'a str,
        }
        let playback_info = props.playback_status.information.as_ref().map(|info| {
            let not_empty = |s: &&str| !s.is_empty();
            Info {
                title: Some(info.title.trim())
                    .filter(not_empty)
                    .unwrap_or("[No Title]"),
                artist: Some(info.artist.trim())
                    .filter(not_empty)
                    .unwrap_or("[No Artist]"),
                album: Some(info.album.trim())
                    .filter(not_empty)
                    .unwrap_or("[No Album]"),
            }
        });
        html! {
            <>
                <div>
                    if let Some(Info { title, .. }) = &playback_info {
                        <span class="title">{ title }</span>
                    } else {
                        { "[No Active Track]" }
                    }
                </div>
                <div>
                    if let Some(Info { artist, album, .. }) = &playback_info {
                        <span>
                            <span class="artist">{ artist }</span>
                            { SEPARATOR }
                            <span class="album">{ album }</span>
                        </span>
                    }
                </div>
            </>
        }
    }
}
