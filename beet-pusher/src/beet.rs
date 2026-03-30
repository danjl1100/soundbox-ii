// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
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
    /// Returns an error if the beet query fails or modifying the network fails
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
/// Returns an error if the beet query fails or modifying the network fails
#[expect(
    clippy::missing_panics_doc,
    reason = "report bug in spigot bucket ref generator"
)]
pub fn fill_buckets<U: BeetRunner>(
    runner: &mut U,
    spigot: &mut bucket_spigot::Network<BeetItem, String>,
) -> Result<(), FillError<U::Error>> {
    use bucket_spigot::{ModifyCmd, path::PathRef};

    let make_err = |kind| FillError { kind };

    let buckets: Vec<_> = spigot
        .get_buckets_needing_fill()
        .map(PathRef::to_owned)
        .collect();

    for bucket in buckets {
        let filters = spigot
            .get_filters(bucket.as_ref())
            .expect("path should be valid for bucket needing fill")
            .into_iter()
            .flat_map(|filter_set| filter_set.iter().cloned())
            .collect::<Vec<_>>();

        let new_contents = BeetItem::list_from_beet_query(runner, filters.iter().cloned())
            .map_err(FillErrorKind::Query)
            .map_err(make_err)?;

        info!("fill bucket {bucket} with {} items", new_contents.len());
        if new_contents.is_empty() {
            warn!(?bucket, ?filters, "empty bucket");
        }

        spigot
            .modify(ModifyCmd::FillBucket {
                bucket,
                new_contents,
            })
            .map_err(FillErrorKind::Modify)
            .map_err(make_err)?;
    }

    Ok(())
}

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
