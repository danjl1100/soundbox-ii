// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! References to static web resource files (with optional development reloading)

use eyre::Context;
use tiny_http::{Request, Response};

/// Constructor for [`StaticFile`]
#[macro_export]
macro_rules! static_file {
    ($path:literal) => {{
        use $crate::static_file::StaticFile;
        const STATIC_FILE: StaticFile = StaticFile {
            path: $path,
            bytes: include_bytes!(concat!("../../static/", $path)),
        };
        STATIC_FILE
    }};
}

/// Reference to a file in the `static` source folder
///
/// Constructed via the [`static_file!`] macro
#[derive(Clone, Copy)]
pub struct StaticFile {
    /// Relative path to the source file
    pub path: &'static str,
    /// Contents of the file at compile time
    pub bytes: &'static [u8],
}
impl StaticFile {
    /// Replies with the file contents as plaintext
    ///
    /// # Errors
    /// Returns an error if the file load or reply fails
    pub fn reply_file(self, request: Request, prefix: Option<&str>) -> eyre::Result<()> {
        self.get_response(prefix)?.reply_to(request)?;
        Ok(())
    }
    /// Replies with the file contents as HTML
    ///
    /// # Errors
    /// Returns an error if the file load or reply fails
    pub fn reply_html(self, request: Request, prefix: Option<&str>) -> eyre::Result<()> {
        self.get_response(prefix)?
            .content_type("text/html")
            .reply_to(request)?;
        Ok(())
    }
    /// Replies with the file contents as CSS
    ///
    /// # Errors
    /// Returns an error if the file load or reply fails
    pub fn reply_css(self, request: Request, prefix: Option<&str>) -> eyre::Result<()> {
        self.get_response(prefix)?
            .content_type("text/css")
            .reply_to(request)?;
        Ok(())
    }
    /// Replies with the file contents as JS
    ///
    /// # Errors
    /// Returns an error if the file load or reply fails
    pub fn reply_js(self, request: Request, prefix: Option<&str>) -> eyre::Result<()> {
        self.get_response(prefix)?
            .content_type("text/javascript")
            .reply_to(request)?;
        Ok(())
    }
    fn get_response(self, prefix: Option<&str>) -> eyre::Result<FileResponse> {
        let Self { path, bytes } = self;
        let file_response = if let Some(prefix) = prefix {
            let path = format!("{prefix}/{path}");
            let file = std::fs::File::open(&path)
                .with_context(|| format!("failed to read path: {path}"))?;
            FileResponse::File(Response::from_file(file))
        } else {
            FileResponse::Data(Response::from_data(bytes))
        };
        Ok(file_response)
    }
}

enum FileResponse {
    File(Response<std::fs::File>),
    Data(Response<std::io::Cursor<Vec<u8>>>),
}
impl FileResponse {
    fn reply_to(self, request: Request) -> std::io::Result<()> {
        match self {
            Self::File(inner) => request.respond(inner),
            Self::Data(inner) => request.respond(inner),
        }
    }
    /// # Panics
    /// Panics if the provided string is non-ascii
    fn content_type(self, content_type: &'static str) -> Self {
        self.with_header("Content-Type", content_type)
            .expect("valid content-type")
    }
    /// Returns the response with the header, or `None` if the header is invalid (non-ascii)
    fn with_header(self, header: &'static str, value: &'static str) -> Option<Self> {
        let header = tiny_http::Header::from_bytes(header, value).ok()?;
        let this = match self {
            Self::File(inner) => Self::File(inner.with_header(header)),
            Self::Data(inner) => Self::Data(inner.with_header(header)),
        };
        Some(this)
    }
}
