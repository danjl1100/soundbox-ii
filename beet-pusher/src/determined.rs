use crate::BeetPath;

/// `Vec<T>` where each modification re-creates a URL cache
pub struct Determined<T> {
    items: Vec<T>,
    urls: Vec<url::Url>,
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
    /// Returns the items
    #[allow(clippy::must_use_candidate)]
    pub fn items(&self) -> &[T] {
        &self.items
    }
    /// Returns the cached URLs
    #[allow(clippy::must_use_candidate)]
    pub fn urls(&self) -> &[url::Url] {
        &self.urls
    }
    #[allow(clippy::must_use_candidate, missing_docs)]
    pub fn len(&self) -> usize {
        self.items.len()
    }
    #[allow(clippy::must_use_candidate, missing_docs)]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
    /// Allow modification of the `Vec<T>`, then clears and rebuilds the URL cache for the new
    /// items
    ///
    /// # Errors
    /// Returns an error if the [`UrlSource`] conversion fails
    #[allow(clippy::missing_panics_doc)]
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
