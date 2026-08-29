// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use crate::BeetPath;

/// `Vec<T>` where each modification re-creates a URL cache
pub struct Determined<T> {
    /// Inner items
    pub mut(self) items: Vec<T>,
    /// Cached items from the [`UrlSource`] used in [`Self::modify`], guaranteed
    /// to match the length of `items`
    pub mut(self) urls: Vec<url::Url>,
}
impl<T> Default for Determined<T> {
    fn default() -> Self {
        Self {
            items: vec![],
            urls: vec![],
        }
    }
}
impl<T> Determined<T>
where
    T: AsRef<BeetPath>,
{
    /// Allow modification of the `Vec<T>`, then clears and rebuilds the URL cache for the new
    /// items
    ///
    /// # Errors
    /// Returns an error if the [`UrlSource`] conversion fails
    #[allow(
        clippy::missing_panics_doc,
        reason = "report bug in internal items/urls pairing"
    )]
    pub fn modify<U, E>(
        &mut self,
        url_source: &impl UrlSource<Error = E>,
        modify_fn: impl FnOnce(&mut Vec<T>) -> U,
    ) -> Result<U, E> {
        let mut result = Ok(modify_fn(&mut self.items));
        match self
            .items
            .iter()
            .map(|item| url_source.get_url(item.as_ref()))
            .collect()
        {
            Ok(new_urls) => self.urls = new_urls,
            Err(err) => {
                // failed to create URLs, clear items for consistent state
                self.items.clear();
                self.urls.clear();
                result = Err(err);
            }
        }
        assert_eq!(
            self.items.len(),
            self.urls.len(),
            "determined items/urls should match lengths"
        );
        result
    }
}

/// Converts [`BeetPath`] to [`url::Url`]
pub trait UrlSource {
    /// Error converting to the URL
    type Error;
    /// Converts [`BeetPath`] to [`url::Url`]
    ///
    /// # Errors
    /// Returns an error if the URL conversion fails
    fn get_url(&self, item_path: &BeetPath) -> Result<url::Url, Self::Error>;
}
impl<F, E> UrlSource for F
where
    F: Fn(&BeetPath) -> Result<url::Url, E>,
{
    type Error = E;
    fn get_url(&self, item_path: &BeetPath) -> Result<url::Url, Self::Error> {
        (self)(item_path)
    }
}
