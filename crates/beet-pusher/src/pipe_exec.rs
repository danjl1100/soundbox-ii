// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Commands from stdin and responses to stdout

#![expect(missing_docs, reason = "TODO while designing API")]

#[derive(Clone, Debug, serde::Deserialize)] // NOTE: not `Serialize`, test should compare plain strings
pub struct CommandIn {
    pub seq: Option<u64>,
    #[serde(flatten)]
    pub cmd: Command,
}
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(tag = "cmd")]
#[serde(rename_all = "snake_case")]
pub enum Command {
    AddNode { parent: String },
    SetFilter { path: String, filters: Vec<String> },
}

pub type ResponseResult = Result<std::convert::Infallible /* TODO */, Error>;

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseOut {
    Error(Error),
}
impl From<ResponseResult> for ResponseOut {
    fn from(value: ResponseResult) -> Self {
        match value {
            Err(e) => Self::Error(e),
        }
    }
}

#[derive(Debug, serde::Serialize, thiserror::Error)] // NOTE: not `Deserialize`, tests should compare plain strings
#[must_use]
#[serde(tag = "kind")]
#[serde(rename_all = "snake_case")]
pub enum Error {
    #[error(transparent)]
    InvalidCommand(ErrorInvalidCommand),
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
