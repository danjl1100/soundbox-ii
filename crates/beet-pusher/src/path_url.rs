// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use crate::{BeetPath, determined::UrlSource};

/// Base for converting [`BeetPath`] to absolute URLs
// TODO add tests for Windows beet-path conversion to URL
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(from = "url::Url")]
pub struct BaseUrl(pub url::Url);
impl BaseUrl {
    // TODO delete if unused
    // pub fn new(url: &str) -> Result<Self, ErrorBase> {
    //     url.parse().map(Self).map_err(|error| ErrorBase {
    //         base_url_str: url.to_string(),
    //         error,
    //     })
    // }
}
impl From<url::Url> for BaseUrl {
    fn from(value: url::Url) -> Self {
        Self(value)
    }
}

impl UrlSource for BaseUrl {
    type Error = ErrorBeetPath;

    fn get_url(&self, item_path: &BeetPath) -> Result<url::Url, ErrorBeetPath> {
        // SOURCE `url::parser::PATH` not public, and somehow not used for `url::Url::join`
        // <https://github.com/servo/rust-url/blob/7492360d4230b67fa0e62794b6fde276525e5f84/url/src/parser.rs#L23>
        const PATH: &percent_encoding::AsciiSet = &percent_encoding::CONTROLS
            .add(b' ')
            .add(b'"')
            .add(b'<')
            .add(b'>')
            .add(b'`')
            //
            .add(b'#')
            .add(b'?')
            .add(b'{')
            .add(b'}');

        let base_url = &self.0;

        let path = item_path.as_str();
        let path = path.strip_prefix('/').unwrap_or(path);
        let path_percentencoded = percent_encoding::utf8_percent_encode(path, PATH).to_string();
        let path = &path_percentencoded;

        let url = base_url.join(path).map_err(|error| ErrorBeetPath {
            item_path: item_path.clone(),
            error,
        })?;

        Ok(url)
    }
}

// TODO delete if unused
// #[derive(Debug)]
// pub(super) struct ErrorBase {
//     base_url_str: String,
//     error: url::ParseError,
// }
// impl std::error::Error for ErrorBase {
//     fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
//         Some(&self.error)
//     }
// }
// impl std::fmt::Display for ErrorBase {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         let Self {
//             base_url_str,
//             error: _,
//         } = self;
//         write!(f, "invalid base URL {base_url_str:?}")
//     }
// }
#[derive(Debug)]
pub struct ErrorBeetPath {
    item_path: BeetPath,
    error: url::ParseError,
}
impl std::error::Error for ErrorBeetPath {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}
impl std::fmt::Display for ErrorBeetPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            item_path,
            error: _,
        } = self;
        write!(f, "invalid beet URL: {item_path:?}")
    }
}

#[cfg(test)]
mod tests {
    use super::BaseUrl;
    use crate::{BeetItem, determined::UrlSource as _};

    #[test]
    fn beet_path_not_fragment() {
        for input in [
            "/path/to/file_containing_#_sign.txt",
            "/path/to/file that contains #hash tag signs and other symbols {},%$#%#$@#?!@",
        ] {
            let item = BeetItem::new_unchecked(0, input.to_string());
            let base = BaseUrl("file:///some/base/".parse().expect("test base url valid"));

            let result = base.get_url(item.get_path()).expect("test item url valid");
            assert_eq!(
                result.fragment(),
                None,
                "should not have fragment for input: {input}"
            );
        }
    }
}
