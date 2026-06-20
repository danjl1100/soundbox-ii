// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Commands from stdin and responses to stdout

#![expect(missing_docs, reason = "TODO while designing API")]

use crate::BeetItem;

pub use bucket_spigot::path::Path as NodePath;

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct RequestSequence(u64);

#[derive(Clone, Debug, serde::Deserialize)] // NOTE: not `Serialize`, test should compare plain strings
pub struct CommandIn {
    pub seq: RequestSequence,
    #[serde(flatten)]
    pub cmd: Command,
}
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(untagged)]
pub enum Command {
    Spigot(SpigotCmd),
    Vlc(VlcCmd),
}
#[derive(Clone, Debug, serde::Deserialize)]
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
#[derive(Clone, Debug, serde::Deserialize)]
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
#[derive(Clone, Debug, serde::Deserialize)]
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
pub type ResponseResultInner = Result<ResponseData, Error>;

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseOut {
    Data {
        reply_to_seq: RequestSequence,
        #[serde(flatten)]
        data: ResponseData,
    },
    Error(Error),
}
impl From<ResponseResult> for ResponseOut {
    fn from(value: ResponseResult) -> Self {
        match value {
            Ok((reply_to_seq, data)) => Self::Data { reply_to_seq, data },
            Err(e) => Self::Error(e),
        }
    }
}

#[derive(Debug, serde::Serialize)]
#[serde(tag = "kind")]
#[serde(rename_all = "snake_case")]
pub enum ResponseData {
    NodeAdded {
        path: NodePath,
    },
    #[serde(rename = "pass")]
    PassNoData, // No additional data
}

#[derive(Debug, serde::Serialize, thiserror::Error)] // NOTE: not `Deserialize`, tests should compare plain strings
#[must_use]
#[serde(tag = "kind", content = "details")]
#[serde(rename_all = "snake_case")]
pub enum Error {
    #[error(transparent)]
    InvalidCommand(ErrorInvalidCommand),
    #[error(transparent)]
    SpigotError(#[serde(serialize_with = "serialize_as_display")] bucket_spigot::ModifyError),
    #[error(transparent)]
    VlcRequest(#[serde(serialize_with = "serialize_as_display")] vlc_http_ureq::Error),
    #[error("internal request operation timed out")]
    InternalTimeout,
}
impl Error {
    pub fn new_invalid_command(command_json: String, source: serde_json::Error) -> Self {
        Self::InvalidCommand(ErrorInvalidCommand {
            command_json,
            source,
        })
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
