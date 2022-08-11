// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

pub use disconnected::Disconnected;
mod disconnected;
pub use playback::Playback;
mod playback;

pub use heartbeat::Heartbeat;
mod heartbeat {
    use std::borrow::Cow;
    use yew::{function_component, html, html::IntoPropValue, Properties};

    use crate::{model, router};

    #[derive(PartialEq)]
    pub struct Data {
        last_heartbeat: Option<shared::Time>,
    }
    #[derive(Properties, PartialEq)]
    pub struct Props {
        pub data: Data,
        pub show_debug: bool,
    }
    impl IntoPropValue<Data> for &model::Data {
        fn into_prop_value(self) -> Data {
            let last_heartbeat = self.last_heartbeat();
            Data { last_heartbeat }
        }
    }

    #[function_component(Heartbeat)]
    pub fn heartbeat(props: &Props) -> Html {
        let Data { last_heartbeat } = props.data;
        html! {
            <div>
                if props.show_debug {
                    <>
                        <router::Link to={router::Route::DebugPanel}>
                            { "Debug" }
                        </router::Link>
                        { " " }
                    </>
                }
                { "Sever last seen: " }
                { last_heartbeat.map_or(Cow::Borrowed("Never"), |t| format!("{t:?}").into()) }
            </div>
        }
    }
}

pub use album_art::AlbumArt;
mod album_art {
    use yew::{function_component, html, html::IntoPropValue, Properties};

    #[derive(PartialEq)]
    pub struct Data {
        hash: u64,
    }
    #[derive(Properties, PartialEq)]
    pub struct Props {
        pub data: Data,
    }
    impl IntoPropValue<Data> for Option<&shared::PlaybackInfo> {
        fn into_prop_value(self) -> Data {
            // NOTE: less-attractive alternative: store all fields in props, and defer
            // calculating hash until after `yew` PartialEq verifies the fields are different,
            // in the `view` function.    (the current implementation seems best)
            if self.is_none() {
                log!("AlbumArt given prop data {self:?}");
            }
            let hash = self.map_or(0, |info| {
                use std::hash::Hasher;
                let mut hasher = twox_hash::XxHash64::with_seed(0);
                let fields = [
                    &info.title,
                    &info.artist,
                    &info.album,
                    &info.date,
                    &info.track_number,
                ];
                log!("AlbumArt fields are: {fields:?}");
                for (idx, field) in fields.iter().enumerate() {
                    hasher.write(field.as_bytes());
                    hasher.write_usize(idx);
                }
                hasher.finish()
            });
            Data { hash }
        }
    }

    #[function_component(AlbumArt)]
    pub fn album_art(Props { data }: &Props) -> Html {
        let Data { hash } = data;
        let src = format!("/v1/art?trick_reload_key={hash}");
        html! {
            <img {src} alt="Album Art" class="keep-true-color" />
        }
    }
}

pub use copying::Copying;
mod copying {
    use crate::router;
    use yew::{function_component, html};

    #[function_component(Copying)]
    pub fn copying() -> Html {
        html! {
            <>
            <h2>{"Copying"}</h2>
            <div>
                <div>{"This program comes with ABSOLUTELY NO WARRANTY."}</div>
                <div>{"This is free software, and you are welcome to redistribute it under certain conditions."}</div>
                { router::link_to_default() }
                <br />
                <br />
                <div>{"See the full license text below:"}</div>
                <div class="legal">
                    <pre>{ shared::license::FULL_LICENSE }</pre>
                </div>
            </div>
            </>
        }
    }
}

pub use player::player;
mod player {
    use crate::{model, view};
    use shared::Command;
    use yew::{html, Callback, Html};

    pub fn player(model: &model::Data, on_command_opt: Callback<Option<Command>>) -> Html {
        html! {
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
        }
    }
}
