// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use super::arb_rng::PanicRng;
use crate::{clap::ModifyCmd as ClapModifyCmd, path::Path, ModifyCmd, ModifyError, Network};
use ::clap::Parser as _;

#[derive(serde::Serialize)]
pub(super) struct Log<T, U>(Vec<Entry<T, U>>);
#[derive(Debug, serde::Serialize)]
pub(super) enum Entry<T, U> {
    BucketsNeedingFill(Vec<Path>),
    Filters(Path, Vec<Vec<U>>),
    ExpectError(String, String),
    Peek(
        #[serde(
            skip_serializing_if = "Option::is_none",
            with = "::serde_with::rust::unwrap_or_skip"
        )]
        Option<u64>,
        Vec<T>,
    ),
    /// Only shown when no values are requested (e.g. [`Command::PeekAssert`])
    PeekEffort(u64),
    Pop(
        #[serde(
            skip_serializing_if = "Option::is_none",
            with = "::serde_with::rust::unwrap_or_skip"
        )]
        Option<u64>,
        Vec<T>,
    ),
    Topology(Topology<usize>),
}

#[derive(Debug, serde::Serialize)]
#[serde(untagged)]
pub(super) enum Topology<T> {
    Leaf(T),
    Node(Vec<Topology<T>>),
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
    Peek {
        #[command(flatten)]
        flags: PeekFlags,
        count: usize,
    },
    PeekAssert {
        #[command(flatten)]
        flags: PeekFlags,
        expected: Vec<T>,
    },
    Topology,
}

#[derive(Clone, Copy, Debug, clap::Args)]
struct PeekFlags {
    #[clap(long)]
    apply: bool,
    #[clap(long)]
    show_effort: bool,
}

impl Network<String, String> {
    pub(super) fn new_strings() -> Self {
        Self::default()
    }
    pub(super) fn new_strings_run_script(commands: &str) -> Log<String, String> {
        Self::new_strings().run_script(commands)
    }
}

impl<T, U> Network<T, U>
where
    T: crate::clap::ArgBounds + Eq,
    U: crate::clap::ArgBounds,
{
    pub(super) fn run_script(&mut self, commands: &str) -> Log<T, U> {
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
            entries.extend(entry);
        }
        if let Some(expect_why) = expect_error {
            panic!("expect_err annotation should be followed by a command: {expect_why}");
        };

        Log(entries)
    }
    pub(super) fn run_script_command(
        &mut self,
        command_str: &str,
    ) -> Result<Option<Entry<T, U>>, Box<dyn std::error::Error>> {
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
                    let mut buckets: Vec<_> = self
                        .get_buckets_needing_fill()
                        .map(Path::to_owned)
                        .collect();
                    buckets.sort();
                    Some(Entry::BucketsNeedingFill(buckets))
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
            Command::Peek { flags, count } => {
                let (effort, peeked) = self.run_peek(count, flags);
                let entry = if flags.apply {
                    Entry::Pop(effort, peeked)
                } else {
                    Entry::Peek(effort, peeked)
                };
                Ok(Some(entry))
            }
            Command::PeekAssert { flags, expected } => {
                let count = expected.len();
                let (effort, peeked) = self.run_peek(count, flags);
                assert_eq!(peeked, expected);

                let entry = flags
                    .apply
                    .then_some(
                        // show `Pop` in log, even when redundant with an assert
                        Entry::Pop(effort, peeked),
                    )
                    .or(
                        // log the effort (if present, e.g. when requested)
                        effort.map(Entry::PeekEffort),
                    );
                Ok(entry)
            }
            Command::Topology => {
                let topology = Topology::new_from_nodes(&self.root);
                Ok(Some(Entry::Topology(topology)))
            }
        }
    }
    fn run_peek(&mut self, count: usize, flags: PeekFlags) -> (Option<u64>, Vec<T>) {
        let peeked = self.peek(&mut PanicRng, count).unwrap();
        let items = peeked
            .items()
            .iter()
            .map(|&x| x.clone())
            .collect::<Vec<_>>();
        let effort = flags.show_effort.then_some(peeked.get_effort_count());
        if flags.apply {
            self.finalize_peeked(peeked.accept_into_inner());
        }
        (effort, items)
    }
}

impl Topology<usize> {
    fn new_from_nodes<T, U>(nodes: &[crate::Child<T, U>]) -> Self {
        let elems = nodes
            .iter()
            .map(|node| match node {
                crate::Child::Bucket(bucket) => Self::Leaf(bucket.len()),
                crate::Child::Joint(joint) => Self::new_from_nodes(&joint.children),
            })
            .collect();
        Self::Node(elems)
    }
}
