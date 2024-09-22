// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use crate::{order::UnknownOrderPath, UnknownPath};

/// Error modifying the [`Network`]
#[allow(clippy::module_name_repetitions)]
pub struct ViewError(ViewErr);
enum ViewErr {
    UnknownPath(UnknownPath),
    UnknownOrderPath(UnknownOrderPath),
    ExcessiveViewDimensions(ExcessiveViewDimensions),
}
impl From<UnknownPath> for ViewError {
    fn from(value: UnknownPath) -> Self {
        Self(ViewErr::UnknownPath(value))
    }
}
impl From<UnknownOrderPath> for ViewError {
    fn from(value: UnknownOrderPath) -> Self {
        Self(ViewErr::UnknownOrderPath(value))
    }
}
impl From<ExcessiveViewDimensions> for ViewError {
    fn from(value: ExcessiveViewDimensions) -> Self {
        Self(ViewErr::ExcessiveViewDimensions(value))
    }
}
impl From<ViewErr> for ViewError {
    fn from(value: ViewErr) -> Self {
        Self(value)
    }
}

pub(super) struct ExcessiveViewDimensions {
    label: &'static str,
    count: usize,
}
impl std::fmt::Display for ExcessiveViewDimensions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { label, count } = self;
        write!(f, "excessive view dimensions ({label} {count})")
    }
}

impl std::error::Error for ViewError {}
impl std::fmt::Display for ViewError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self(inner) = self;
        match inner {
            ViewErr::UnknownPath(err) => write!(f, "{err}"),
            ViewErr::UnknownOrderPath(err) => {
                write!(f, "{err}")
            }
            ViewErr::ExcessiveViewDimensions(err) => write!(f, "{err}"),
        }
    }
}
impl std::fmt::Debug for ViewError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ViewError({self})")
    }
}

pub(crate) fn count(label: &'static str, count: usize) -> Result<u32, ExcessiveViewDimensions> {
    count
        .try_into()
        .map_err(|_| ExcessiveViewDimensions { label, count })
}
