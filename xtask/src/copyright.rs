// Copyright (C) 2021-2025  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Checks the copyright notice for modified files

use crate::Fix;
use eyre::{Context, ContextCompat as _};
use std::{
    collections::HashSet,
    io::{BufRead as _, BufReader},
};

const PREFIX: &str = "// Copyright (C) 2021-";
const SUFFIX: &str =
    "  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details";

/// Verifies the copyright notice for modified files in the GIT index, fixing if requested
///
/// # Errors
/// Returns an error if the GIT I/O fails, errors are present without permission to fix,
/// or I/O fails while performing fixes.
pub fn checks(fix: Option<Fix>) -> eyre::Result<()> {
    let current_year = jiff::Zoned::now().year().cast_unsigned();

    let repo = git2::Repository::open_from_env().context("failed to open GIT repo")?;
    let Some(workdir) = repo.workdir() else {
        eyre::bail!("GIT workdir not found")
    };

    let mut staged_files_list = find_staged_files_list(&repo, |path| {
        std::path::Path::new(path)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("rs"))
    })?;

    let index = repo.index()?;

    let mut files_need_update = vec![];
    for entry in index.iter() {
        let id = entry.id;
        let path = String::from_utf8(entry.path).context("non-UTF8 filename in index")?;

        if !staged_files_list.remove(&path) {
            // index entry is not an eligible change
            continue;
        }

        let index_object = repo
            .find_object(id, None)
            .context("index object not found in repo")?;
        // dbg!((id, &path, &index_object));

        let index_content = index_object
            .as_blob()
            .context("index object is not a blob")?
            .content();
        let copyright_result = check_copyright_header(index_content);
        match copyright_result {
            Ok(found) => {
                let Some(fix_needed) = found.as_fix(current_year) else {
                    // already correct
                    continue;
                };

                if let Some(Fix) = fix {
                    let path_absolute = workdir.join(&path);

                    eprintln!("rewriting {path} ...");

                    // fix the copyright header
                    let worktree_content = std::fs::read(&path_absolute)
                        .with_context(|| format!("failed to read: {path}"))?;
                    let dest = std::fs::File::options()
                        .write(true)
                        .open(&path_absolute)
                        .with_context(|| format!("failed to open for writing: {path}"))?;
                    fix_needed.apply(&worktree_content, current_year, dest)?;

                    eprintln!("copyright updated: {path}");
                    continue;
                }

                if let CopyrightResult::Copyright { year, .. } = found {
                    // no fix - error invalid year
                    eprintln!(
                        "copyright header out of date (found {year}, expected {current_year}): {path}"
                    );
                } else {
                    // no fix - missing
                    eprintln!("copyright header missing: {path}");
                }
            }
            Err(e) => {
                return Err(eyre::eyre!(e))
                    .with_context(|| format!("failed to read copyright header from: {path}"));
            }
        }

        files_need_update.push(path);
    }

    if !staged_files_list.is_empty() {
        eyre::bail!("staged file not found in the index: {staged_files_list:#?}")
    }

    if !files_need_update.is_empty() {
        let len = files_need_update.len();
        let plural = if len == 1 { "" } else { "s" };

        eprintln!("Need copyright updated in {len} file{plural}:");
        for file in &files_need_update {
            eprintln!("\t{file}");
        }

        eyre::bail!("Incorrect copyright in {len} file{plural} above");
    }

    Ok(())
}

fn find_staged_files_list(
    repo: &git2::Repository,
    filter_fn: impl Fn(&str) -> bool,
) -> eyre::Result<HashSet<String>> {
    let statuses = repo.statuses(Some(
        git2::StatusOptions::new()
            .show(git2::StatusShow::Index)
            .no_refresh(true),
    ))?;

    statuses
        .iter()
        .filter_map(|entry| {
            let status = entry.status();
            if status.is_index_deleted() {
                // ignore deleted files, nothing to check
                return None;
            }

            let Some(path_in_workdir) = entry.path() else {
                return Some(Err(eyre::eyre!(
                    "invalid (non-UTF8?) path in GIT status entry {:?}",
                    entry.path_bytes()
                )));
            };

            filter_fn(path_in_workdir)
                .then(|| path_in_workdir.to_string())
                .map(Ok)
        })
        .collect()
}

#[derive(Debug)]
enum CopyrightResult {
    Empty,
    NoMatch,
    Copyright { line: String, year: u16 },
}
#[derive(Debug)]
enum CopyrightFix {
    UpdateOld { expect_line: String },
    InsertNew,
}
impl CopyrightResult {
    fn as_fix(&self, current_year: u16) -> Option<CopyrightFix> {
        match self {
            CopyrightResult::Copyright { year, .. } if *year == current_year => None,
            CopyrightResult::Copyright { line, year: _ } => Some(CopyrightFix::UpdateOld {
                expect_line: line.clone(),
            }),
            CopyrightResult::Empty | CopyrightResult::NoMatch => Some(CopyrightFix::InsertNew),
        }
    }
}
impl CopyrightFix {
    fn apply(
        self,
        worktree_content: &[u8],
        current_year: u16,
        mut dest: impl std::io::Write,
    ) -> eyre::Result<()> {
        let mut old_content = BufReader::new(worktree_content);

        match self {
            CopyrightFix::UpdateOld { expect_line } => {
                let mut line = String::new();
                loop {
                    line.clear();
                    let read = old_content.read_line(&mut line)?;
                    if read == 0 {
                        eyre::bail!(
                            "file does not match index, refusing to overwrite (`git add -p` to stage latest version, then retry)"
                        )
                    }

                    if line == expect_line {
                        // match, replace in common code below
                        break;
                    }

                    write!(&mut dest, "{line}")?;
                }

                // write header
                writeln!(&mut dest, "{PREFIX}{current_year}{SUFFIX}")?;

                // write the remaining lines
                loop {
                    line.clear();
                    let read = old_content.read_line(&mut line)?;
                    if read == 0 {
                        break;
                    }
                    write!(&mut dest, "{line}")?;
                }
            }
            CopyrightFix::InsertNew => {
                // write header
                writeln!(&mut dest, "{PREFIX}{current_year}{SUFFIX}")?;
                // copy remainder of the content
                std::io::copy(&mut old_content, &mut dest)?;
            }
        }

        Ok(())
    }
}

/// Returns the [`CopyrightResult`] if present
///
/// # Errors
/// Returns an IO error if reading the slice fails (e.g. non-UTF8 bytes)
///
/// Within the success, case, returns the first line (if any) that did not match the copyright pattern
fn check_copyright_header(content: &[u8]) -> std::io::Result<CopyrightResult> {
    let mut lines = BufReader::new(content);

    let mut empty = true;
    let mut line = String::new();
    loop {
        line.clear();
        let read = lines.read_line(&mut line)?;
        if read == 0 {
            break;
        }
        empty = false;

        let Some(year_and_suffix) = line.strip_prefix(PREFIX) else {
            continue;
        };
        let Some(year) = year_and_suffix.trim_end().strip_suffix(SUFFIX) else {
            continue;
        };

        let Some(year) = year.parse().ok() else {
            continue;
        };
        return Ok(CopyrightResult::Copyright { line, year });
    }
    if empty {
        Ok(CopyrightResult::Empty)
    } else {
        Ok(CopyrightResult::NoMatch)
    }
}
