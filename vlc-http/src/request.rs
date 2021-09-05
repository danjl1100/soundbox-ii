//! HTTP-specific primitives (interchange for test purposes)

use super::command::{ArtRequestIntent, CmdArgs, RequestIntent};

pub use http::{
    uri::{Authority, InvalidUri, PathAndQuery, Uri},
    Method,
};

/// VLC backend request information
#[must_use]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RequestInfo {
    pub path_and_query: PathAndQuery,
    pub method: Method,
}
impl From<&RequestIntent<'_, '_>> for RequestInfo {
    fn from(intent: &RequestIntent<'_, '_>) -> Self {
        const STATUS_JSON: &str = "/requests/status.json";
        const PLAYLIST_JSON: &str = "/requests/playlist.json";
        let path_and_query = match intent {
            RequestIntent::Status(Some(CmdArgs { command, args })) => {
                Self::format_cmd_args(STATUS_JSON, command, args)
            }
            RequestIntent::Playlist(Some(CmdArgs { command, args })) => {
                Self::format_cmd_args(PLAYLIST_JSON, command, args)
            }
            RequestIntent::Status(None) => PathAndQuery::from_static(STATUS_JSON),
            RequestIntent::Playlist(None) => PathAndQuery::from_static(PLAYLIST_JSON),
        };
        Self {
            path_and_query,
            method: Method::GET,
        }
    }
}
impl From<&ArtRequestIntent> for RequestInfo {
    fn from(intent: &ArtRequestIntent) -> Self {
        const ART: &str = "/art";
        let ArtRequestIntent { id } = intent;
        let path_and_query = id.as_ref().map_or_else(
            || PathAndQuery::from_static(ART),
            |id| {
                Self::format_path_query(
                    ART,
                    &Self::query_builder().append_pair("item", id).finish(),
                )
            },
        );
        Self {
            path_and_query,
            method: Method::GET,
        }
    }
}
impl RequestInfo {
    fn query_builder() -> form_urlencoded::Serializer<'static, String> {
        form_urlencoded::Serializer::new(String::new())
    }
    fn format_cmd_args(path: &str, command: &str, args: &[(&str, String)]) -> PathAndQuery {
        let query = Self::query_builder()
            .append_pair("command", command)
            .extend_pairs(args)
            .finish();
        Self::format_path_query(path, &query)
    }
    fn format_path_query(path: &str, query: &str) -> PathAndQuery {
        format!("{path}?{query}", path = path, query = query)
            .parse()
            .expect("valid urlencoded args")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn encodes_status_request() {
        let empty = RequestIntent::Status(Some(CmdArgs {
            command: "sentinel_command",
            args: vec![],
        }));
        assert_eq!(
            RequestInfo::from(&empty),
            RequestInfo {
                path_and_query: "/requests/status.json?command=sentinel_command"
                    .parse()
                    .unwrap(),
                method: Method::GET,
            }
        );
        //
        let with_args = RequestIntent::Status(Some(CmdArgs {
            command: "second",
            args: vec![
                ("first", "this".to_string()),
                ("then", "something else".to_string()),
            ],
        }));
        assert_eq!(
            RequestInfo::from(&with_args),
            RequestInfo {
                path_and_query:
                    "/requests/status.json?command=second&first=this&then=something+else"
                        .parse()
                        .unwrap(),
                method: Method::GET,
            }
        );
    }
    #[test]
    fn encodes_playlist_request() {
        let empty = RequestIntent::Playlist(Some(CmdArgs {
            command: "do_something",
            args: vec![],
        }));
        assert_eq!(
            RequestInfo::from(&empty),
            RequestInfo {
                path_and_query: "/requests/playlist.json?command=do_something"
                    .parse()
                    .unwrap(),
                method: Method::GET,
            }
        );
        //
        let with_args = RequestIntent::Playlist(Some(CmdArgs {
            command: "ditherous",
            args: vec![
                ("everything", "is".to_string()),
                ("awesome", "!!#$%".to_string()),
            ],
        }));
        assert_eq!(RequestInfo::from(&with_args), RequestInfo {
            path_and_query: "/requests/playlist.json?command=ditherous&everything=is&awesome=%21%21%23%24%25".parse().unwrap(),
            method: Method::GET,
        });
    }
    #[test]
    fn encodes_art_request() {
        let empty = ArtRequestIntent { id: None };
        assert_eq!(
            RequestInfo::from(&empty),
            RequestInfo {
                path_and_query: "/art".parse().unwrap(),
                method: Method::GET,
            }
        );
        //
        let with_id = ArtRequestIntent {
            id: Some("sentinel_ID".to_string()),
        };
        assert_eq!(
            RequestInfo::from(&with_id),
            RequestInfo {
                path_and_query: "/art?item=sentinel_ID".parse().unwrap(),
                method: Method::GET,
            }
        );
    }
    #[test]
    fn encodes_playback_status_request() {
        let status = RequestIntent::Status(None);
        assert_eq!(
            RequestInfo::from(&status),
            RequestInfo {
                path_and_query: "/requests/status.json".parse().unwrap(),
                method: Method::GET,
            },
        );
    }
    #[test]
    fn encodes_playlist_status_request() {
        let playlist = RequestIntent::Playlist(None);
        assert_eq!(
            RequestInfo::from(&playlist),
            RequestInfo {
                path_and_query: "/requests/playlist.json".parse().unwrap(),
                method: Method::GET,
            },
        );
    }
}
