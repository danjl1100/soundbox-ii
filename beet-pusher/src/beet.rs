pub use self::item::BeetItem;
pub use self::path::BeetPath;
use tracing::{info, warn};

mod item;
mod path;
mod query;

/// Fills any pending buckets in the spigot using beet
///
/// # Errors
/// Returns an error if the beet query fails or modifying the network fails
#[allow(clippy::missing_panics_doc)]
pub fn fill_buckets(
    spigot: &mut bucket_spigot::Network<BeetItem, String>,
) -> Result<(), FillError> {
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

        let new_contents = BeetItem::list_from_beet_query(filters.iter().cloned())
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
pub struct FillError {
    kind: FillErrorKind,
}
#[derive(Debug)]
enum FillErrorKind {
    Modify(bucket_spigot::ModifyError),
    Query(self::query::Error),
}
impl std::error::Error for FillError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            FillErrorKind::Modify(inner) => Some(inner),
            FillErrorKind::Query(inner) => Some(inner),
        }
    }
}
impl std::fmt::Display for FillError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { kind } = self;
        match kind {
            FillErrorKind::Modify(_) => write!(f, "failed to modify network"),
            FillErrorKind::Query(_) => write!(f, "failed to query beet"),
        }
    }
}
