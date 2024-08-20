// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use super::{
    arb_rng::{PanicRng, RngHolder},
    fake_rng,
};
use crate::{clap::ModifyCmd as ClapModifyCmd, path::Path, ModifyCmd, ModifyError, Network};
use ::clap::Parser as _;
use arbitrary::Unstructured;
use std::fmt::Write as _;

#[derive(serde::Serialize)]
pub(super) struct Log<T, U>(Vec<Entry<T, U>>);
#[derive(Debug, serde::Serialize)]
pub(super) enum Entry<T, U> {
    BucketsNeedingFill(
        String,
        #[serde(skip_serializing_if = "Vec::is_empty")] Vec<Path>,
    ),
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
    RngRemaining(String),
}

#[derive(Debug, serde::Serialize)]
#[serde(untagged)]
pub(super) enum Topology<T> {
    Leaf(T),
    LeafEmpty,
    NodeList(Vec<Topology<T>>),
    // NOTE: "Map", but still need to maintain insertion order
    NodeMap(Vec<(T, Topology<T>)>),
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
    Topology {
        kind: Option<TopologyKind>,
    },
    EnableRng {
        bytes_hex: Vec<String>,
    },
}

#[derive(Clone, Copy, Debug, Default, clap::ValueEnum)]
enum TopologyKind {
    #[default]
    ItemCount,
    Weights,
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
        let mut rng_holder = RngHolder::default();

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

            let result = self.run_script_command(cmd, &mut rng_holder);

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

        let rng_remaining = rng_holder.get_bytes();
        if !rng_remaining.is_empty() {
            let mut s = String::new();
            for remaining in rng_remaining {
                write!(&mut s, "{remaining:02x}").expect("infallible");
            }
            entries.push(Entry::RngRemaining(s));
        }

        Log(entries)
    }
    pub(super) fn run_script_command(
        &mut self,
        command_str: &str,
        rng_holder: &mut RngHolder,
    ) -> Result<Option<Entry<T, U>>, Box<dyn std::error::Error>> {
        let cmd = Command::<T, U>::try_parse_from(command_str.split_whitespace())?;
        match cmd {
            Command::Modify { cmd } => {
                let cmd = cmd.into();
                let output_buckets = matches!(
                    &cmd,
                    ModifyCmd::AddBucket { .. }
                        | ModifyCmd::FillBucket { .. }
                        | ModifyCmd::SetFilters { .. }
                );
                self.modify(cmd)?;

                let entry = if output_buckets {
                    let mut buckets: Vec<_> = self
                        .get_buckets_needing_fill()
                        .map(Path::to_owned)
                        .collect();
                    buckets.sort();
                    Some(Entry::BucketsNeedingFill(command_str.to_owned(), buckets))
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
                let (effort, peeked) = self.run_peek(count, flags, rng_holder);
                let entry = if flags.apply {
                    Entry::Pop(effort, peeked)
                } else {
                    Entry::Peek(effort, peeked)
                };
                Ok(Some(entry))
            }
            Command::PeekAssert { flags, expected } => {
                let count = expected.len();
                let (effort, peeked) = self.run_peek(count, flags, rng_holder);
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
            Command::Topology { kind } => {
                let topology = match kind.unwrap_or_default() {
                    TopologyKind::ItemCount => Topology::new_from_nodes(&self.root),
                    TopologyKind::Weights => Topology::new_from_weights(&self.root),
                };
                Ok(Some(Entry::Topology(topology)))
            }
            Command::EnableRng { bytes_hex } => match rng_holder.set_bytes(&bytes_hex) {
                Ok(()) => Ok(None),
                Err(Some(err)) => Err(err.into()),
                Err(None) => Err(DuplicateRngInitError.into()),
            },
        }
    }
    fn run_peek(
        &mut self,
        count: usize,
        flags: PeekFlags,
        rng_holder: &mut RngHolder,
    ) -> (Option<u64>, Vec<T>) {
        let peeked = self.peek_test_rng(count, rng_holder).unwrap();
        let items = peeked
            .items()
            .iter()
            .map(|&x| x.clone())
            .collect::<Vec<_>>();
        let effort = flags.show_effort.then_some(peeked.get_effort_count());
        if flags.apply {
            let accepted = peeked.accept_into_inner();
            self.finalize_peeked(accepted);
        }
        (effort, items)
    }
    fn peek_test_rng(
        &mut self,
        count: usize,
        rng_holder: &mut RngHolder,
    ) -> Result<crate::order::Peeked<'_, T>, rand::Error> {
        let bytes = rng_holder.get_bytes();
        if bytes.is_empty() {
            self.peek(&mut PanicRng, count)
        } else {
            let mut u = Unstructured::new(bytes);
            let mut rng = fake_rng(&mut u);

            let result = self.peek(&mut rng, count);

            // clear used bytes from `rng_holder`
            let remaining = u.len();
            rng_holder.truncate_from_left(remaining);

            result
        }
    }
}

impl Topology<usize> {
    fn new_from_nodes<T, U>(nodes: &crate::ChildVec<crate::Child<T, U>>) -> Self {
        let elems = nodes
            .children()
            .iter()
            .map(|node| match node {
                crate::Child::Bucket(bucket) => Self::Leaf(bucket.items.len()),
                crate::Child::Joint(joint) => Self::new_from_nodes(&joint.next),
            })
            .collect();
        Self::NodeList(elems)
    }
    fn new_from_weights<T, U>(nodes: &crate::ChildVec<crate::Child<T, U>>) -> Self {
        let weights = nodes.weights();
        let map = nodes
            .children()
            .iter()
            .enumerate()
            .map(|(index, node)| {
                let weight = if weights.is_empty() {
                    1
                } else {
                    weights[index]
                        .try_into()
                        .expect("weight should fit in platform's usize")
                };
                let target = match node {
                    crate::Child::Bucket(_) => Self::LeafEmpty,
                    crate::Child::Joint(joint) => Self::new_from_weights(&joint.next),
                };
                (weight, target)
            })
            .collect();
        Self::NodeMap(map)
    }
}

#[derive(Debug)]
struct DuplicateRngInitError;
impl std::fmt::Display for DuplicateRngInitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RNG can only be enabled once")
    }
}
impl std::error::Error for DuplicateRngInitError {}
