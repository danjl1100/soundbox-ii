// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! HTTP-level primitives (interchange for test purposes)

use crate::{
    command::{VolumePercent256, VolumePercentDelta256},
    Command,
};
use std::borrow::Cow;

#[cfg(test)]
mod tests;

/// VLC HTTP endpoint information to execute a [`Command`]
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[must_use]
pub struct Endpoint {
    path_and_query: Cow<'static, str>,
}
impl Endpoint {
    /// Returns the combined HTTP path and query string for the endpoint
    #[must_use]
    pub fn get_path_and_query(&self) -> &str {
        &self.path_and_query
    }
    /// Returns the HTTP method for the endpoint
    #[must_use]
    pub fn get_method(&self) -> http::Method {
        // NOTE: this is a function for future expansion purposes
        http::Method::GET
    }
}

mod endpoint_args {
    use super::Endpoint;
    use std::borrow::Cow;
    use std::fmt::Write;

    const PATH_STATUS_JSON: &str = "/requests/status.json";
    const PATH_PLAYLIST_JSON: &str = "/requests/playlist.json";

    impl Endpoint {
        pub(crate) fn query_status() -> Endpoint {
            EndpointArgs::new(PATH_STATUS_JSON, None).finish()
        }
        pub(crate) fn query_playlist() -> Endpoint {
            EndpointArgs::new(PATH_PLAYLIST_JSON, None).finish()
        }
    }

    /// Builder for [`Endpoint`]
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub(crate) struct EndpointArgs {
        path_and_query: Cow<'static, str>,
        pending_query_prefix: Option<()>,
    }
    impl EndpointArgs {
        fn new(path: &'static str, command: Option<&'static str>) -> Self {
            let mut this = Self {
                path_and_query: path.into(),
                pending_query_prefix: Some(()),
            };
            if let Some(command) = command {
                this = this.append("command", command);
            }
            this
        }
        pub fn new_status(command: &'static str) -> Self {
            Self::new(PATH_STATUS_JSON, Some(command))
        }
        pub fn new_playlist(command: &'static str) -> Self {
            Self::new(PATH_PLAYLIST_JSON, Some(command))
        }
        // TODO
        // pub fn new_art(id: &str) -> Self {
        //     const PATH_ART: &str = "/art";
        //     Self::new(PATH_ART, None).append("item", id)
        // }
        //
        pub fn append(self, key: &str, value: &str) -> Self {
            let key = urlencoding::encode(key);
            let value = urlencoding::encode(value);
            self.append_raw(&key, &value)
        }
        pub fn append_url(self, key: &str, value: &url::Url) -> Self {
            let key = urlencoding::encode(key);
            // `url::Url` already applies URL encoding,
            // and VLC does not understand a doubly-encoded URL
            let value = value.as_str();
            self.append_raw(&key, value)
        }
        fn append_raw(mut self, key: &str, value: &str) -> Self {
            let sep = if self.pending_query_prefix.take().is_some() {
                "?"
            } else {
                "&"
            };

            {
                let dest: &mut String = self.path_and_query.to_mut();
                write!(dest, "{sep}{key}={value}")
            }
            .expect("string write is infallible");

            self
        }
        pub fn finish(self) -> Endpoint {
            let Self {
                path_and_query,
                pending_query_prefix: _,
            } = self;
            Endpoint { path_and_query }
        }
    }
}

impl Command {
    // TODO
    // /// Creates a request endpoint for the current art
    // pub fn art_endpoint(id: &str) -> Endpoint {
    //     endpoint_args::EndpointArgs::new_art(id).finish()
    // }
    /// Creates a request endpoint for the command
    pub fn into_endpoint(self) -> Endpoint {
        self.into()
    }
}
impl From<Command> for Endpoint {
    /// Creates a request for the specified command
    fn from(command: Command) -> Self {
        use endpoint_args::EndpointArgs as Args;
        match command {
            Command::PlaylistAdd { url } => {
                Args::new_playlist("in_enqueue").append_url("input", &url)
            }
            Command::PlaylistDelete { item_id } => {
                Args::new_playlist("pl_delete").append("id", &item_id.to_string())
            }
            Command::PlaylistPlay { item_id } => {
                let mut args = Args::new_status("pl_play");
                if let Some(item_id) = item_id {
                    args = args.append("id", &item_id);
                }
                args
            }
            Command::PlaybackResume => Args::new_status("pl_forceresume"),
            Command::PlaybackPause => Args::new_status("pl_forcepause"),
            Command::PlaybackStop => Args::new_status("pl_stop"),
            Command::SeekNext => Args::new_status("pl_next"),
            Command::SeekPrevious => Args::new_status("pl_previous"),
            Command::SeekTo { seconds } => {
                Args::new_status("seek").append("val", &seconds.to_string())
            }
            Command::SeekRelative { seconds_delta } => {
                Args::new_status("seek").append("val", &seconds_delta.to_string())
            }
            Command::Volume { percent } => Args::new_status("volume")
                .append("val", &VolumePercent256::from(percent).to_string()),
            Command::VolumeRelative { percent_delta } => Args::new_status("volume").append(
                "val",
                &VolumePercentDelta256::from(percent_delta).to_string(),
            ),
            Command::ToggleRandom => Args::new_status("pl_random"),
            Command::ToggleRepeatOne => Args::new_status("pl_repeat"),
            Command::ToggleLoopAll => Args::new_status("pl_loop"),
            Command::PlaybackSpeed { speed } => {
                Args::new_status("rate").append("val", &speed.to_string())
            }
        }
        .finish()
    }
}
