// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Checks the copyright notice for modified files

use crate::{CmdSettings, WriteOutput};
use eyre::Context;
use std::io::{BufRead as _, BufReader};

const PREFIX: &str = "// Copyright (C) 2021-";
const SUFFIX: &str =
    "  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details";

/// The user wants to write (at last) copyright headers
#[derive(Clone, Copy)]
pub struct WriteCopyright(WriteOutput);
impl From<WriteOutput> for WriteCopyright {
    fn from(value: WriteOutput) -> Self {
        Self(value)
    }
}

/// Verifies the copyright notice for all rust source files in `crates/`, fixing if requested
///
/// # Errors
///
/// Returns an error if errors are present without permission to fix, or I/O
/// fails while performing fixes.
pub fn checks(_cmd: &CmdSettings, fix: Option<WriteCopyright>) -> eyre::Result<()> {
    let current_year = jiff::Zoned::now().year().cast_unsigned();

    let crates_dir = crate::project_root();

    let mut files_display_need_update = vec![];
    for entry in walkdir::WalkDir::new(crates_dir) {
        let entry = entry?;
        let path = entry.path();

        let Some(extension) = path.extension() else {
            // not a rust file
            continue;
        };
        if extension != "rs" {
            // not a rust file
            continue;
        }

        let copyright_result = {
            let content = std::fs::File::open(path)
                .with_context(|| format!("failed to open: {path}", path = path.display()))?;
            let content = &mut BufReader::new(content);
            check_copyright_header(content)
        };
        match copyright_result {
            Ok(found) => {
                let Some(fix_needed) = found.as_fix(current_year) else {
                    // already correct
                    continue;
                };

                if let Some(WriteCopyright(WriteOutput { .. })) = fix {
                    eprintln!("rewriting {path} ...", path = path.display());

                    // fix the copyright header
                    let worktree_content = std::fs::read(path).with_context(|| {
                        format!("failed to read: {path}", path = path.display())
                    })?;
                    let dest = std::fs::File::options()
                        .write(true)
                        .open(path)
                        .with_context(|| {
                            format!("failed to open for writing: {path}", path = path.display())
                        })?;
                    fix_needed.apply(&worktree_content, current_year, dest)?;

                    eprintln!("copyright updated: {path}", path = path.display());
                    continue;
                }

                if let CopyrightResult::Copyright { year, .. } = found {
                    // no fix - error invalid year
                    eprintln!(
                        "copyright header out of date (found {year}, expected {current_year}): {path}",
                        path = path.display()
                    );
                } else {
                    // no fix - missing
                    eprintln!("copyright header missing: {path}", path = path.display());
                }
            }
            Err(e) => {
                return Err(eyre::eyre!(e)).with_context(|| {
                    format!(
                        "failed to read copyright header from: {path}",
                        path = path.display()
                    )
                });
            }
        }

        files_display_need_update.push(path.display().to_string());
    }

    if !files_display_need_update.is_empty() {
        let len = files_display_need_update.len();
        let plural = if len == 1 { "" } else { "s" };

        eprintln!("Need copyright updated in {len} file{plural}:");
        for file in &files_display_need_update {
            eprintln!("\t{file}");
        }

        eyre::bail!("Incorrect copyright in {len} file{plural} above");
    }

    Ok(())
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
fn check_copyright_header(
    content: &mut BufReader<impl std::io::Read>,
) -> std::io::Result<CopyrightResult> {
    let mut empty = true;
    let mut line = String::new();
    loop {
        line.clear();
        let read = content.read_line(&mut line)?;
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
