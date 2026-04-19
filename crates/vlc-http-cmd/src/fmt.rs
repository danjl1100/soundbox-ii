// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

//! Helper for formatting types

use url::Url;

/// Compact debug format for a [`Url`]
#[derive(Clone, PartialEq, Eq, serde::Serialize)]
#[serde(transparent)]
pub struct DebugUrl(pub Url);
impl std::fmt::Debug for DebugUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let r = DebugUrlRef(&self.0);
        <DebugUrlRef as std::fmt::Debug>::fmt(&r, f)
    }
}
impl AsRef<Url> for DebugUrl {
    fn as_ref(&self) -> &Url {
        &self.0
    }
}

/// Compact debug format for a [`Url`]
#[derive(Clone, PartialEq, Eq)]
pub struct DebugUrlRef<'a>(pub &'a Url);
impl std::fmt::Debug for DebugUrlRef<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // debug `Url` as Display (built-in Debug is far too verbose)
        write!(f, "Url(\"")?;
        <Url as std::fmt::Display>::fmt(self.0, f)?;
        write!(f, "\")")?;
        Ok(())
    }
}
impl AsRef<Url> for DebugUrlRef<'_> {
    fn as_ref(&self) -> &Url {
        self.0
    }
}
