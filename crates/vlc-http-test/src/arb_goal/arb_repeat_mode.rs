// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use vlc_http::goal::RepeatMode;

/// Arbitrary [`RepeatMode`]
#[derive(Clone, Debug, arbitrary::Arbitrary)]
#[expect(missing_docs, reason = "self explanatory")]
pub enum ArbRepeatMode {
    Off,
    All,
    One,
}
impl From<RepeatMode> for ArbRepeatMode {
    fn from(value: RepeatMode) -> Self {
        match value {
            RepeatMode::Off => Self::Off,
            RepeatMode::All => Self::All,
            RepeatMode::One => Self::One,
        }
    }
}
impl From<ArbRepeatMode> for RepeatMode {
    fn from(value: ArbRepeatMode) -> Self {
        match value {
            ArbRepeatMode::Off => Self::Off,
            ArbRepeatMode::All => Self::All,
            ArbRepeatMode::One => Self::One,
        }
    }
}
