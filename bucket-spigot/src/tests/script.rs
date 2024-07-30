// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use crate::{clap::ModifyCmd as ClapModifyCmd, path::Path, ModifyCmd, ModifyError, Network};
use ::clap::Parser as _;

#[derive(serde::Serialize)]
pub(super) struct Log<U>(Vec<Entry<U>>);
#[derive(Debug, serde::Serialize)]
pub(super) enum Entry<U> {
    BucketsNeedingFill(Vec<Path>),
    Filters(Path, Vec<Vec<U>>),
    ExpectError(String, String),
}

#[derive(clap::Parser)]
#[clap(no_binary_name = true)]
enum Command<T, U>
where
    T: crate::clap::ArgBounds,
    U: crate::clap::ArgBounds,
{
    Modify {
        #[clap(subcommand)]
        cmd: ClapModifyCmd<T, U>,
    },
    GetFilters {
        path: Path,
    },
}

impl Network<String, String> {
    pub(super) fn new_strings() -> Self {
        Self::default()
    }
}

impl<T, U> Network<T, U>
where
    T: crate::clap::ArgBounds,
    U: crate::clap::ArgBounds,
{
    pub(super) fn run_script(&mut self, commands: &'static str) -> Log<U> {
        let mut entries = vec![];

        let mut expect_error = None;
        for (index, cmd) in commands.lines().enumerate() {
            let cmd = cmd.trim();
            let debug_line = || format!("{cmd:?} (line {number})", number = index + 1);

            if cmd.starts_with("!!expect_error") {
                let expect_why = debug_line();
                let None = expect_error.replace(expect_why) else {
                    panic!("duplicate expect_err annotation: {}", debug_line());
                };
                continue;
            }
            if cmd.is_empty() || cmd.starts_with('#') {
                continue;
            }

            let result = self.run_script_command(cmd);

            let entry = if let Some(expect_error_why) = expect_error.take() {
                // expect error
                let error_str = result.expect_err(&expect_error_why).to_string();
                // log error value
                Some(Entry::ExpectError(cmd.to_owned(), error_str))
            } else {
                // expect success
                result.unwrap_or_else(|err| {
                    // print unexpected error
                    panic!("error running command: {}\n{err}", debug_line())
                })
            };
            if let Some(entry) = entry {
                entries.push(entry);
            }
        }
        if let Some(expect_why) = expect_error {
            panic!("expect_err annotation should be followed by a command: {expect_why}");
        };

        Log(entries)
    }
    pub(super) fn run_script_command(
        &mut self,
        command_str: &'static str,
    ) -> Result<Option<Entry<U>>, Box<dyn std::error::Error>> {
        let cmd = Command::<T, U>::try_parse_from(command_str.split_whitespace())?;
        match cmd {
            Command::Modify { cmd } => {
                let cmd = cmd.into();
                let output_buckets = matches!(
                    &cmd,
                    ModifyCmd::AddBucket { .. } | ModifyCmd::FillBucket { .. }
                );
                self.modify(cmd)?;

                let entry = if output_buckets {
                    let buckets = self.get_buckets_needing_fill();
                    Some(Entry::BucketsNeedingFill(buckets.to_owned()))
                } else {
                    None
                };
                Ok(entry)
            }
            Command::GetFilters { path } => {
                let filters = self.get_filters(path.clone()).map_err(ModifyError::from)?;
                let filters = filters
                    .iter()
                    .map(|&filter_set| filter_set.to_owned())
                    .collect();
                Ok(Some(Entry::Filters(path, filters)))
            }
        }
    }
}
