// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use crate::command::{VolumePercent, VolumePercentDelta};

/// [`VolumePercent`] converted to a 256-based scale
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct VolumePercent256(u16);
impl VolumePercent256 {
    /// Convert the 256-based value into the equivalent precentage
    pub(crate) fn unchecked_to_percent(based_256: u16) -> u16 {
        let percent = f32::from(based_256) / Self::PERCENT_TO_256;
        #[expect(clippy::cast_possible_truncation, reason = "conversion factor is <1.0")]
        #[expect(clippy::cast_sign_loss, reason = "u16 is always non-negative")]
        {
            percent.round() as u16
        }
    }
}

/// [`VolumePercentDelta`] converted to a 256-based scale
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct VolumePercentDelta256 {
    is_negative: bool,
    magnitude: VolumePercent256,
}
impl From<VolumePercentDelta> for VolumePercentDelta256 {
    fn from(delta: VolumePercentDelta) -> Self {
        let VolumePercentDelta(value) = delta;
        Self {
            is_negative: value < 0,
            magnitude: delta.unsigned_abs().into(),
        }
    }
}

impl std::fmt::Display for VolumePercent256 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self(value) = self;
        write!(f, "{value}")
    }
}
impl std::fmt::Display for VolumePercentDelta256 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            is_negative,
            magnitude,
        } = *self;

        let sign_char = if is_negative { '-' } else { '+' };
        let VolumePercent256(magnitude) = magnitude;
        write!(f, "{sign_char}{magnitude}")
    }
}

impl VolumePercent256 {
    pub(super) const PERCENT_TO_256: f32 = (256.0 / 100.0);
}
impl From<VolumePercent> for VolumePercent256 {
    fn from(percent: VolumePercent) -> Self {
        // VolumePercent enforces bounds 0-300 (inclusive)
        let VolumePercent(percent) = percent;

        // result is 0-768 (inclusive), comfortably fits in u16
        let based_256 = f32::from(percent) * Self::PERCENT_TO_256;
        #[expect(
            clippy::cast_possible_truncation,
            reason = "target size comfortably fits 0-768 (inclusive)"
        )]
        #[expect(clippy::cast_sign_loss, reason = "value is always non-negative")]
        {
            Self(based_256.round() as u16)
        }
    }
}
