// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

//! Helper for formatting types

#[derive(Clone, PartialEq, Eq, serde::Serialize)]
#[serde(transparent)]
pub(crate) struct DebugUrl(pub url::Url);
impl std::fmt::Debug for DebugUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let r = DebugUrlRef(&self.0);
        <DebugUrlRef as std::fmt::Debug>::fmt(&r, f)
    }
}
impl AsRef<url::Url> for DebugUrl {
    fn as_ref(&self) -> &url::Url {
        &self.0
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct DebugUrlRef<'a>(pub &'a url::Url);
impl std::fmt::Debug for DebugUrlRef<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // debug `url::Url` as Display (built-in Debug is far too verbose)
        write!(f, "Url(\"")?;
        <url::Url as std::fmt::Display>::fmt(self.0, f)?;
        write!(f, "\")")?;
        Ok(())
    }
}
impl AsRef<url::Url> for DebugUrlRef<'_> {
    fn as_ref(&self) -> &url::Url {
        self.0
    }
}
