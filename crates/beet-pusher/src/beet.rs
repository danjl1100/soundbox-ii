// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Queries items from `beet`
pub use self::item::BeetItem;
pub use self::path::BeetPath;
pub use self::query::{BeetCommand, BeetRunner};
use crate::BeetPusher;
use tracing::{info, warn};

mod item;
mod path;
mod query;

impl<R> BeetPusher<'_, R> {
    /// Fills any pending buckets in the inner spigot using the specified [`BeetRunner`]
    ///
    /// # Errors
    ///
    /// Returns an error if the beet query fails or modifying the network fails
    ///
    /// # See Also
    ///
    /// [`crate::beet::fill_buckets`]
    pub fn fill_buckets<U: BeetRunner>(
        &mut self,
        runner: &mut U,
    ) -> Result<(), FillError<U::Error>> {
        fill_buckets(runner, self.get_spigot_mut())
    }
}

/// Fills any pending buckets in the spigot using the specified [`BeetRunner`]
///
/// # Errors
///
/// Returns an error if the beet query fails or modifying the network fails
///
/// # See Also
///
/// 1. [`crate::beet::get_bucket_fill_needs`]
/// 2. [`crate::beet::BucketQueryNeed::query`]
/// 3. [`crate::beet::apply_bucket_fill_results`]
pub fn fill_buckets<U: BeetRunner>(
    runner: &mut U,
    spigot: &mut bucket_spigot::Network<BeetItem, String>,
) -> Result<(), FillError<U::Error>> {
    let needs = get_bucket_fill_needs(spigot);

    // collect is required to end the `set_filter` borrow (inside `needs`)
    // before storing the results into the spigot
    let query_results: Vec<_> = needs.into_iter().map(|need| need.query(runner)).collect();

    apply_bucket_fill_results(query_results, spigot)
}

/// Returns the bucket queries needing to fill buckets
#[expect(
    clippy::missing_panics_doc,
    reason = "report bug in spigot bucket ref generator"
)]
pub fn get_bucket_fill_needs(
    spigot: &mut bucket_spigot::Network<BeetItem, String>,
) -> impl Iterator<Item = BucketQueryNeed> + use<'_> {
    use bucket_spigot::path::PathRef;

    // NOTE: The intermediate `collect` is needed because:
    // 1. get_buckets_needing_fill requires `&mut Network` to update an internal cache
    let buckets: Vec<_> = spigot
        .get_buckets_needing_fill()
        .map(PathRef::to_owned)
        .collect();

    // 2. get_filters requires `&Network`
    buckets.into_iter().map(|bucket| {
        let filters = spigot
            .get_filters(bucket.as_ref())
            .expect("path should be valid for bucket needing fill")
            .into_iter()
            .flat_map(|filter_set| filter_set.iter().cloned())
            .collect();
        BucketQueryNeed { bucket, filters }
    })
}

/// Applies the results from [`BucketQueryNeed::query`] to the specified spigot
///
/// # Errors
///
/// Returns an error if the query results contain an error or are invalid
pub fn apply_bucket_fill_results<T>(
    query_results: impl IntoIterator<Item = BucketQueryResult<T>>,
    spigot: &mut bucket_spigot::Network<BeetItem, String>,
) -> Result<(), FillError<T>> {
    let make_err = |kind| FillError { kind };

    for query_result in query_results {
        let query_result = query_result
            .map_err(FillErrorKind::Query)
            .map_err(make_err)?;

        spigot
            .modify(query_result.into())
            .map_err(FillErrorKind::Modify)
            .map_err(make_err)?;
    }

    Ok(())
}

/// Query needed from beet, created from [`get_bucket_fill_needs`]
pub struct BucketQueryNeed {
    bucket: bucket_spigot::path::Path,
    filters: Vec<String>,
}
/// Output of a bucket's beet query, from [`BucketQueryNeed::query`]
pub struct BucketItems {
    bucket: bucket_spigot::path::Path,
    new_contents: Vec<BeetItem>,
}
/// Result from [`BucketQueryNeed::query`]
pub type BucketQueryResult<T> = Result<BucketItems, self::query::Error<T>>;
impl BucketQueryNeed {
    /// Executes the beet query with the specified runner
    ///
    /// # Errors
    ///
    /// Returns an error if the beet command fails or returns invalid results
    pub fn query<U: BeetRunner>(self, runner: &mut U) -> BucketQueryResult<U::Error> {
        let Self { bucket, filters } = self;

        let new_contents = BeetItem::list_from_beet_query(runner, filters.iter().cloned())?;

        info!("fill bucket {bucket} with {} items", new_contents.len());
        if new_contents.is_empty() {
            warn!(?bucket, ?filters, "empty bucket");
        }

        Ok(BucketItems {
            bucket,
            new_contents,
        })
    }
}
impl<U> From<BucketItems> for bucket_spigot::ModifyCmd<BeetItem, U>
where
    U: bucket_spigot::clap::ArgBounds,
{
    fn from(value: BucketItems) -> Self {
        let BucketItems {
            bucket,
            new_contents,
        } = value;
        Self::FillBucket {
            bucket,
            new_contents,
        }
    }
}

/// Error querying beet or filling buckets
#[derive(Debug)]
pub struct FillError<T> {
    kind: FillErrorKind<T>,
}
#[derive(Debug)]
enum FillErrorKind<T> {
    Modify(bucket_spigot::ModifyError),
    Query(self::query::Error<T>),
}
impl<T> std::error::Error for FillError<T>
where
    T: std::fmt::Debug,
    self::query::Error<T>: std::error::Error + 'static,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            FillErrorKind::Modify(inner) => Some(inner),
            FillErrorKind::Query(inner) => Some(inner),
        }
    }
}
impl<T> std::fmt::Display for FillError<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { kind } = self;
        match kind {
            FillErrorKind::Modify(_) => write!(f, "failed to modify network"),
            FillErrorKind::Query(_) => write!(f, "failed to query beet"),
        }
    }
}
