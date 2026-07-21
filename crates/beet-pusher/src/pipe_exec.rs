// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Commands from stdin and responses to stdout

#![expect(missing_docs, reason = "TODO while designing API")]

use crate::BeetItem;

pub use bucket_spigot::path::Path as NodePath;

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct RequestSequence(u64);
impl From<u64> for RequestSequence {
    fn from(seq: u64) -> Self {
        Self(seq)
    }
}
impl std::fmt::Display for RequestSequence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self(inner) = self;
        write!(f, "{inner}")
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct CommandIn {
    pub seq: RequestSequence,
    #[serde(flatten)]
    pub cmd: Command,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum Command {
    Spigot(SpigotCmd),
    Vlc(VlcCmd),
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "cmd")]
#[serde(rename_all = "snake_case")]
pub enum SpigotCmd {
    AddNode {
        parent: NodePath,
        node_kind: NodeKind,
    },
    SetFilters {
        path: NodePath,
        new_filters: Vec<String>,
    },
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    // TODO: Joint,
    Bucket,
}
impl From<SpigotCmd> for bucket_spigot::ModifyCmd<BeetItem, String> {
    fn from(value: SpigotCmd) -> Self {
        use bucket_spigot::ModifyCmd;
        match value {
            SpigotCmd::AddNode { parent, node_kind } => match node_kind {
                // TODO: NodeKind::Joint => ModifyCmd::AddJoint { parent },
                NodeKind::Bucket => ModifyCmd::AddBucket { parent },
            },
            SpigotCmd::SetFilters { path, new_filters } => {
                ModifyCmd::SetFilters { path, new_filters }
            }
        }
    }
}

/// Subset of [`vlc_http::command::Command`] allowed while [`crate::BeetPusher`]
/// manages the playlist and playback mode
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "cmd")]
#[serde(rename_all = "snake_case")]
pub enum VlcCmd {
    SeekNext,
}
impl From<VlcCmd> for vlc_http::Command {
    fn from(value: VlcCmd) -> Self {
        match value {
            VlcCmd::SeekNext => Self::SeekNext,
        }
    }
}

/// Sequence and Result
///
/// [`ResponseResultInner`] with [`RequestSequence`] in the `Ok(_)` case
// TODO: likely want optional `RequestSequence` in the Error case, too (if the
// request was valid enough to contain a sequence)
pub type ResponseResult = Result<(RequestSequence, ResponseData), Error>;
/// Inner Result only
pub type ResponseResultInner = Result<ResponseData, ErrorKind>;

pub type ResponseOutDe = ResponseOut<serde_json::Value>;

/// Serializable rich-error version of [`ResponseOutDe`]
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseOut<K = ErrorKind> {
    Data {
        reply_to_seq: RequestSequence,
        #[serde(flatten)]
        data: ResponseData,
    },
    Error(Error<K>),
}
impl From<ResponseResult> for ResponseOut {
    fn from(value: ResponseResult) -> Self {
        match value {
            Ok((reply_to_seq, data)) => Self::Data { reply_to_seq, data },
            Err(error) => Self::Error(error),
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind")]
#[serde(rename_all = "snake_case")]
pub enum ResponseData {
    NodeAdded {
        path: NodePath,
    },
    #[serde(rename = "pass")]
    PassNoData, // No additional data
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[must_use]
#[serde(rename_all = "snake_case")]
pub struct Error<K = ErrorKind> {
    #[serde(skip_serializing_if = "Option::is_none")]
    reply_to_seq: Option<RequestSequence>,
    #[serde(flatten)]
    kind: K,
}
#[derive(Debug, serde::Serialize, thiserror::Error)] // NOTE: not `Deserialize`, tests should compare plain strings
#[must_use]
#[serde(tag = "kind", content = "details")]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    #[error(transparent)]
    InvalidCommand(#[from] ErrorInvalidCommand),
    #[error(transparent)]
    SpigotError(
        #[from]
        #[serde(serialize_with = "serialize_as_display")]
        bucket_spigot::ModifyError,
    ),
    #[error(transparent)]
    VlcRequest(
        #[from]
        #[serde(serialize_with = "serialize_as_display")]
        vlc_http_ureq::Error,
    ),
    #[error("internal request operation timed out")]
    InternalTimeout,
}
impl Error {
    pub fn new(seq: RequestSequence, kind: ErrorKind) -> Self {
        Self {
            reply_to_seq: Some(seq),
            kind,
        }
    }
    pub fn new_invalid_command(command_json: String, source: serde_json::Error) -> Self {
        Self {
            reply_to_seq: None,
            kind: ErrorInvalidCommand {
                command_json,
                source,
            }
            .into(),
        }
    }
}
impl<K> Error<K> {
    #[must_use]
    pub fn get_reply_to_seq(&self) -> Option<RequestSequence> {
        self.reply_to_seq
    }
    pub fn into_inner(self) -> K {
        self.kind
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        let Self { reply_to_seq, kind } = self;
        match reply_to_seq {
            Some(_) => Some(kind), // source if Some
            None => kind.source(), // transparent if None
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { reply_to_seq, kind } = self;
        match reply_to_seq {
            Some(seq) => write!(f, "request seq={seq} failed"), // source if Some
            None => write!(f, "{kind}"),                        // transparent if None
        }
    }
}

#[derive(Debug, serde::Serialize, thiserror::Error)]
#[error("invalid command JSON: {command_json:?}")]
pub struct ErrorInvalidCommand {
    command_json: String,
    #[serde(skip)]
    source: serde_json::Error,
}

fn serialize_as_display<T, S>(value: &T, s: S) -> Result<S::Ok, S::Error>
where
    T: std::fmt::Display,
    S: serde::Serializer,
{
    let display = value.to_string();
    s.serialize_str(&display)
}
