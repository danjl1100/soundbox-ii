//! References to static web resource files (with optional development reloading)

use eyre::Context;
use tiny_http::Response;

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
    pub fn reply_file(
        self,
        request: tiny_http::Request,
        debug_path_prefix: Option<&str>,
    ) -> eyre::Result<()> {
        self.get_response(debug_path_prefix)?.reply_to(request)?;
        Ok(())
    }
    /// Replies with the file contents as HTML
    ///
    /// # Errors
    /// Returns an error if the file load or reply fails
    #[allow(clippy::missing_panics_doc)] // panic indicates bug
    pub fn reply_html(
        self,
        request: tiny_http::Request,
        debug_path_prefix: Option<&str>,
    ) -> eyre::Result<()> {
        self.get_response(debug_path_prefix)?
            .with_header(
                tiny_http::Header::from_bytes("Content-Type", "text/html").expect("valid header"),
            )
            .reply_to(request)?;
        Ok(())
    }
    fn get_response(self, debug_path_prefix: Option<&str>) -> eyre::Result<FileResponse> {
        let Self { path, bytes } = self;
        let file_response = if let Some(prefix) = debug_path_prefix {
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
    fn reply_to(self, request: tiny_http::Request) -> std::io::Result<()> {
        match self {
            Self::File(inner) => request.respond(inner),
            Self::Data(inner) => request.respond(inner),
        }
    }
    fn with_header(self, header: tiny_http::Header) -> Self {
        match self {
            Self::File(inner) => Self::File(inner.with_header(header)),
            Self::Data(inner) => Self::Data(inner.with_header(header)),
        }
    }
}
