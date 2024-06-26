// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! HTTP-level response primitives

use std::io::Read;

pub use playback::Status as PlaybackStatus;
mod playback;

pub use playlist::Info as PlaylistInfo;
pub mod playlist;

#[cfg(test)]
mod tests;

/// Parsed response from VLC
#[derive(Clone, Debug)]
#[cfg_attr(test, derive(serde::Serialize))]
pub struct Response {
    pub(crate) inner: ResponseInner,
}
#[derive(Clone, Debug)]
#[cfg_attr(test, derive(serde::Serialize))]
pub(crate) enum ResponseInner {
    PlaylistInfo(PlaylistInfo),
    PlaybackStatus(PlaybackStatus),
}

#[derive(serde::Deserialize)]
#[serde(untagged)]
enum ResponseJSON {
    PlaylistInfo(playlist::InfoJSON),
    PlaybackStatus(playback::StatusJSON),
}

impl std::str::FromStr for Response {
    type Err = ParseError;
    fn from_str(s: &str) -> Result<Self, ParseError> {
        let response_json: ResponseJSON = serde_json::from_str(s)?;
        Ok(response_json.into())
    }
}
impl Response {
    /// Parse the VLC response from the specified bytes
    ///
    /// # Errors
    /// Returns an error if the response is invalid
    pub fn from_slice(b: &[u8]) -> Result<Self, ParseError> {
        let response_json: ResponseJSON = serde_json::from_slice(b)?;
        Ok(response_json.into())
    }
    /// Parse the VLC response from the specified reader
    ///
    /// # Errors
    /// Returns an error if the response is invalid
    pub fn from_reader<R>(reader: R) -> Result<Self, ParseError>
    where
        R: Read,
    {
        let response_json: ResponseJSON = serde_json::from_reader(reader)?;
        Ok(response_json.into())
    }
}

impl From<ResponseJSON> for Response {
    fn from(value: ResponseJSON) -> Self {
        match value {
            ResponseJSON::PlaylistInfo(info) => Self {
                inner: ResponseInner::PlaylistInfo(PlaylistInfo::new(info)),
            },
            ResponseJSON::PlaybackStatus(status) => Self {
                inner: ResponseInner::PlaybackStatus(status.into()),
            },
        }
    }
}

/// Error in parsing a VLC response
#[derive(Debug)]
pub struct ParseError {
    serde_json_err: serde_json::Error,
}
impl From<serde_json::Error> for ParseError {
    fn from(value: serde_json::Error) -> Self {
        Self {
            serde_json_err: value,
        }
    }
}
impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { serde_json_err } = self;
        write!(f, "invalid json: {serde_json_err}")
    }
}
impl std::error::Error for ParseError {}
