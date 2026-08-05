// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use std::collections::{BTreeMap, HashSet};

use cargo_metadata::Package;
use eyre::Context as _;

use crate::TypedResult;

const CHECK_FNS: &[fn(&Package) -> Option<String>] = &[
    // rustfmt hint
    check_authors,
    check_license,
    check_publish,
];

fn check_authors(package: &Package) -> Option<String> {
    const AUTHORS: &[&str] = &["Daniel Lambert <danjl1100@gmail.com>"];

    let authors = &package.authors;
    (authors != AUTHORS).then(|| format!("expected common authors, found: {authors:?}"))
}

fn check_license(package: &Package) -> Option<String> {
    const LICENSE: &str = "GPL-3.0-or-later";

    let license = &package.license;
    (license.as_deref() != Some(LICENSE))
        .then(|| format!("expected common license, found: {license:?}"))
}

fn check_publish(package: &Package) -> Option<String> {
    // NOTE:
    // - `publish = true` translates to `null`
    // - `publish = false` translates to `[]` (shows here as `Some([])`)
    const PUBLISH: &[&str] = &[];

    let publish = &package.publish;
    let publish_strs: Option<Vec<_>> = publish
        .as_ref()
        .map(|elems| elems.iter().map(|s| &**s).collect());

    (publish_strs.as_deref() != Some(PUBLISH))
        .then(|| format!("expected no-publish, found: {publish:?}"))
}

pub(super) fn check_crates_metadata() -> TypedResult<()> {
    let metadata = cargo_metadata::MetadataCommand::new()
        .exec()
        .context("failed to exec cargo-metadata command")?;

    let workspace_package_ids: HashSet<_> = HashSet::from_iter(metadata.workspace_members);

    let errors_by_package = metadata
        .packages
        .into_iter()
        .filter_map(|package| {
            if !workspace_package_ids.contains(&package.id) {
                return None;
            }

            let errors: Vec<_> = CHECK_FNS
                .iter()
                .filter_map(|check_fn| check_fn(&package))
                .collect();

            if errors.is_empty() {
                return None;
            }

            Some((package.name.to_string(), errors))
        })
        .collect();

    CrateMetadataErrors::new(errors_by_package).map_err(|e| eyre::eyre!(e))?;

    Ok(())
}

#[derive(Debug)]
struct CrateMetadataErrors(BTreeMap<String, Vec<String>>);
impl CrateMetadataErrors {
    fn new(errors_by_package: BTreeMap<String, Vec<String>>) -> Result<(), Self> {
        if errors_by_package.is_empty() {
            return Ok(());
        }
        Err(Self(errors_by_package))
    }
}
impl std::error::Error for CrateMetadataErrors {}
impl std::fmt::Display for CrateMetadataErrors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self(errors) = self;
        write!(f, "Incorrect crate metadata:\n{errors:#?}")
    }
}
