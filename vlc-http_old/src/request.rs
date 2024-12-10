// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! HTTP-specific primitives (interchange for test purposes)

use crate::http_client::intent::{ArtRequestIntent, CmdArgs, PlaylistIntent, StatusIntent};
use std::fmt::Write;

pub use http::{uri::PathAndQuery, Method};

/// VLC backend request information
#[must_use]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RequestInfo {
    pub path_and_query: PathAndQuery,
    pub method: Method,
}
impl From<&StatusIntent> for RequestInfo {
    fn from(intent: &StatusIntent) -> Self {
        const STATUS_JSON: &str = "/requests/status.json";
        Self {
            path_and_query: Self::format_cmd_args(STATUS_JSON, &intent.0),
            method: Method::GET,
        }
    }
}
impl From<&PlaylistIntent> for RequestInfo {
    fn from(intent: &PlaylistIntent) -> Self {
        const PLAYLIST_JSON: &str = "/requests/playlist.json";
        Self {
            path_and_query: Self::format_cmd_args(PLAYLIST_JSON, &intent.0),
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
            |id| Self::format_path_query(ART, &QueryBuilder::new().append("item", id).finish()),
        );
        Self {
            path_and_query,
            method: Method::GET,
        }
    }
}
impl RequestInfo {
    #[expect(clippy::ref_option)]
    fn format_cmd_args(path: &'static str, cmd_args: &Option<CmdArgs>) -> PathAndQuery {
        cmd_args.as_ref().map_or_else(
            || PathAndQuery::from_static(path),
            |CmdArgs { command, args }| {
                let query = QueryBuilder::new()
                    .append("command", command)
                    .extend(args)
                    .finish();
                Self::format_path_query(path, &query)
            },
        )
    }
    fn format_path_query(path: &str, query: &str) -> PathAndQuery {
        format!("{path}?{query}")
            .parse()
            .expect("valid urlencoded args")
    }
}

/// Builds URI query strings
#[derive(Default)]
struct QueryBuilder(String);
impl QueryBuilder {
    fn new() -> Self {
        Self::default()
    }
    fn append(mut self, key: &str, value: &str) -> Self {
        let sep = if self.0.is_empty() { "" } else { "&" };
        let key = urlencoding::encode(key);
        let value = urlencoding::encode(value);
        write!(self.0, "{sep}{key}={value}").expect("string write succeeds");
        self
    }
    fn extend<'a, I, T, U>(mut self, elems: I) -> Self
    where
        I: IntoIterator<Item = &'a (T, U)>,
        T: AsRef<str> + 'a,
        U: AsRef<str> + 'a,
    {
        for (key, value) in elems {
            self = self.append(key.as_ref(), value.as_ref());
        }
        self
    }
    fn finish(self) -> String {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn encodes_status_request() {
        let empty = StatusIntent(Some(CmdArgs {
            command: "sentinel_command",
            args: vec![],
        }));
        assert_eq!(
            RequestInfo::from(&empty),
            RequestInfo {
                path_and_query: "/requests/status.json?command=sentinel_command"
                    .parse()
                    .expect("uri"),
                method: Method::GET,
            }
        );
        //
        let with_args = StatusIntent(Some(CmdArgs {
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
                    "/requests/status.json?command=second&first=this&then=something%20else"
                        .parse()
                        .expect("uri"),
                method: Method::GET,
            }
        );
    }
    #[test]
    fn encodes_playlist_request() {
        let empty = PlaylistIntent(Some(CmdArgs {
            command: "do_something",
            args: vec![],
        }));
        assert_eq!(
            RequestInfo::from(&empty),
            RequestInfo {
                path_and_query: "/requests/playlist.json?command=do_something"
                    .parse()
                    .expect("uri"),
                method: Method::GET,
            }
        );
        //
        let with_args = PlaylistIntent(Some(CmdArgs {
            command: "ditherous",
            args: vec![
                ("everything", "is".to_string()),
                ("awesome", "!!#$%".to_string()),
                ("with", "some spaces thrown in".to_string()),
            ],
        }));
        assert_eq!(RequestInfo::from(&with_args), RequestInfo {
            path_and_query: "/requests/playlist.json?command=ditherous&everything=is&awesome=%21%21%23%24%25&with=some%20spaces%20thrown%20in".parse().expect("uri"),
            method: Method::GET,
        });
    }
    #[test]
    fn encodes_art_request() {
        let empty = ArtRequestIntent { id: None };
        assert_eq!(
            RequestInfo::from(&empty),
            RequestInfo {
                path_and_query: "/art".parse().expect("uri"),
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
                path_and_query: "/art?item=sentinel_ID".parse().expect("uri"),
                method: Method::GET,
            }
        );
    }
    #[test]
    fn encodes_playback_status_request() {
        let status = StatusIntent(None);
        assert_eq!(
            RequestInfo::from(&status),
            RequestInfo {
                path_and_query: "/requests/status.json".parse().expect("uri"),
                method: Method::GET,
            },
        );
    }
    #[test]
    fn encodes_playlist_status_request() {
        let playlist = PlaylistIntent(None);
        assert_eq!(
            RequestInfo::from(&playlist),
            RequestInfo {
                path_and_query: "/requests/playlist.json".parse().expect("uri"),
                method: Method::GET,
            },
        );
    }
}
