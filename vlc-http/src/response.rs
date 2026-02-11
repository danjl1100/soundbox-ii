// Copyright (C) 2021-2025  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! HTTP-level response primitives

use std::io::Read;

pub use playback::Status as PlaybackStatus;
mod playback;

pub use playlist::Info as PlaylistInfo;
pub mod playlist;

#[cfg(test)]
mod tests;

/// Parsed response from VLC
#[derive(Clone)]
#[cfg_attr(test, derive(serde::Serialize))]
pub struct Response {
    pub(crate) inner: ResponseInner,
}
#[derive(Clone)]
#[cfg_attr(test, derive(serde::Serialize))]
pub(crate) enum ResponseInner {
    PlaylistInfo(PlaylistInfo),
    PlaybackStatus(Box<PlaybackStatus>),
}

#[derive(serde::Deserialize)]
#[serde(untagged)]
enum ResponseJSON {
    PlaylistInfo(playlist::InfoJSON),
    PlaybackStatus(Box<playback::StatusJSON>),
}

impl std::fmt::Debug for Response {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { inner } = self;
        match inner {
            ResponseInner::PlaylistInfo(info) => <_ as std::fmt::Debug>::fmt(info, f),
            ResponseInner::PlaybackStatus(status) => <_ as std::fmt::Debug>::fmt(status, f),
        }
    }
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
            ResponseJSON::PlaybackStatus(status) => {
                let status = Box::new((*status).into());
                Self {
                    inner: ResponseInner::PlaybackStatus(status),
                }
            }
        }
    }
}

/// Error in parsing a VLC response
#[derive(Debug)]
pub struct ParseError {
    source: serde_json::Error,
}
impl From<serde_json::Error> for ParseError {
    fn from(source: serde_json::Error) -> Self {
        Self { source }
    }
}
impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { source: _ } = self;
        write!(f, "invalid VLC response json")
    }
}
impl std::error::Error for ParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        let Self { source } = self;
        Some(source)
    }
}
