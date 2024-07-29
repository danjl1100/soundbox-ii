// soundbox-ii/filter-buckets Item accumulations for sequencing *don't keep your sounds boxed up*
// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

//! A [`Network`] provides a sequence of items, first tentatively then permanently.
//!
//! A central "spigot" (root node) has paths to a network of *joints* (non-leaf nodes), ending
//! at a series of *buckets* (leaf nodes).
//!
//! The user *peeks* a specific number of items from the spigot to view a hypothetical sequence of items.
//! The user *advances* the spigot to "use up" the items, and progress the ordering logic.
//!
//! *Joints* control the sequence of items from buckets to arrive at the spigot.
//!
//! Both *joints* and *buckets* may have one or more *filters* to inform how to fill the buckets with items.
//!
//! Modifying a *joint* queues downstream buckets to be *refilled*.
//! The user provides a list of items to fill each bucket based on the sequence of filters passed when
//! walking from the spigot (root node) to the bucket.
//!

use path::Path;

pub mod clap;
pub mod path;

/// Group of buckets with a central spigot
#[derive(Clone, Debug, Default)]
pub struct Network<T, U> {
    root: Vec<Child<T, U>>,
    buckets_needing_fill: Vec<Path>,
}
impl<T, U> Network<T, U> {
    /// Returns a proposed sequence of items leaving the spigot.
    ///
    /// NOTE: Need to finalize the peeked items to progress the [`Network`] state beyond those
    /// peeked items (depending on the child-ordering involved)
    ///
    /// # Errors
    /// Returns any errors reported by the provided [`rand::Rng`] instance
    #[allow(unused)] // TODO
    pub fn peek<'a, R: rand::Rng + ?Sized>(
        &self,
        rng: &mut R,
        length: usize,
    ) -> Result<&'a [T], rand::Error> {
        // TODO
        // rng.try_fill_bytes(&mut [0])?;
        Ok(&[])
    }
    /// Modify the network topology
    ///
    /// # Errors
    /// Returns an error if the command does not match the current network state
    pub fn modify(&mut self, cmd: ModifyCmd<T, U>) -> Result<(), ModifyError> {
        match cmd {
            ModifyCmd::AddBucket { parent } => {
                self.add_child(Child::bucket(), parent)?;
                Ok(())
            }
            ModifyCmd::AddJoint { parent } => {
                self.add_child(Child::joint(), parent)?;
                Ok(())
            }
            ModifyCmd::DeleteEmpty { path } => self.delete_empty(path),
            ModifyCmd::FillBucket {
                bucket,
                new_contents,
            } => self.set_bucket_items(new_contents, bucket),
            ModifyCmd::SetJointFilters { joint, new_filters } => {
                self.set_joint_filters(new_filters, joint)
            }
        }
    }
    /// Returns the paths to buckets needing to be filled (e.g. filters may have changed)
    #[must_use]
    pub fn get_buckets_needing_fill(&self) -> &[Path] {
        &self.buckets_needing_fill
    }
    /// Returns the filters for the specified path
    ///
    /// NOTE: Returns an empty set for the root path, as the spigot has no filters
    ///
    /// # Errors
    ///
    /// Returns an error if the path is unknown
    pub fn get_filters(&self, path: Path) -> Result<Vec<&[U]>, UnknownPath> {
        let mut filter_groups = Vec::new();

        let mut current_children = Some(&self.root);

        for next_index in &path {
            let Some(next_child) = current_children.and_then(|c| c.get(next_index)) else {
                return Err(UnknownPath(path));
            };
            current_children = match next_child {
                Child::Bucket(_) => None,
                Child::Joint(joint) => {
                    if !joint.filters.is_empty() {
                        filter_groups.push(&joint.filters[..]);
                    }
                    Some(&joint.children)
                }
            };
        }

        Ok(filter_groups)
    }

    fn add_child(&mut self, child: Child<T, U>, parent_path: Path) -> Result<Path, ModifyError> {
        let mut current_children = &mut self.root;

        for next_index in &parent_path {
            let Some(next_child) = current_children.get_mut(next_index) else {
                return Err(UnknownPath(parent_path).into());
            };
            current_children = match next_child {
                Child::Bucket(_) => {
                    return Err(CannotAddToBucket(parent_path).into());
                }
                Child::Joint(joint) => &mut joint.children,
            };
        }

        let child_index = current_children.len();
        let is_bucket = matches!(child, Child::Bucket(_));

        // add child
        current_children.push(child);

        // build child path
        let child_path = {
            let mut path = parent_path;
            path.push(child_index);
            path
        };

        // queue for refilling new bucket
        if is_bucket {
            self.buckets_needing_fill.push(child_path.clone());
        }

        Ok(child_path)
    }
    fn delete_empty(&mut self, path: Path) -> Result<(), ModifyError> {
        let mut current_children = &mut self.root;

        let Some((final_index, parent_path)) = path.as_ref().split_last() else {
            return Err(ModifyErr::DeleteRoot.into());
        };

        for next_index in parent_path {
            let Some(next_child) = current_children.get_mut(next_index) else {
                return Err(UnknownPath(path).into());
            };
            current_children = match next_child {
                Child::Bucket(_) => {
                    return Err(UnknownPath(path).into());
                }
                Child::Joint(joint) => &mut joint.children,
            };
        }

        let Some(target_elem) = current_children.get(final_index) else {
            return Err(UnknownPath(path).into());
        };

        match target_elem {
            Child::Bucket(bucket) if !bucket.is_empty() => {
                Err(ModifyErr::DeleteNonemptyBucket(CannotDeleteNonempty(path)).into())
            }
            Child::Joint(joint) if !joint.children.is_empty() => {
                Err(ModifyErr::DeleteNonemptyJoint(CannotDeleteNonempty(path)).into())
            }
            Child::Bucket(_) | Child::Joint(_) => {
                current_children.remove(final_index);
                Ok(())
            }
        }
    }
    fn set_bucket_items(
        &mut self,
        new_contents: Vec<T>,
        bucket_path: Path,
    ) -> Result<(), ModifyError> {
        let mut current_children = &mut self.root;

        let mut bucket_path_iter = bucket_path.iter();
        let dest_contents = loop {
            let Some(next_index) = bucket_path_iter.next() else {
                return Err(ModifyErr::FillJoint.into());
            };
            let Some(next_child) = current_children.get_mut(next_index) else {
                return Err(UnknownPath(bucket_path).into());
            };
            current_children = match next_child {
                Child::Bucket(bucket) => break bucket,
                Child::Joint(joint) => &mut joint.children,
            };
        };

        self.buckets_needing_fill
            .retain(|path| *path != bucket_path);

        *dest_contents = new_contents;
        Ok(())
    }
    fn set_joint_filters(
        &mut self,
        new_filters: Vec<U>,
        joint_path: Path,
    ) -> Result<(), ModifyError> {
        let mut current_children = &mut self.root;
        let mut dest_filters = None;
        for next_index in &joint_path {
            let Some(next_child) = current_children.get_mut(next_index) else {
                return Err(UnknownPath(joint_path).into());
            };
            current_children = match next_child {
                Child::Bucket(_) => {
                    return Err(CannotAddToBucket(joint_path).into());
                }
                Child::Joint(joint) => {
                    dest_filters = Some(&mut joint.filters);
                    &mut joint.children
                }
            };
        }

        if let Some(dest_filters) = dest_filters {
            *dest_filters = new_filters;
            Ok(())
        } else {
            Err(ModifyErr::FilterRoot)?
        }
    }
}

#[derive(Clone, Debug)]
enum Child<T, U> {
    Bucket(Vec<T>),
    Joint(Joint<T, U>),
}
#[derive(Clone, Debug)]
struct Joint<T, U> {
    children: Vec<Child<T, U>>,
    filters: Vec<U>,
}

impl<T, U> Child<T, U> {
    fn bucket() -> Self {
        Self::Bucket(Vec::new())
    }
    fn joint() -> Self {
        Self::Joint(Joint {
            filters: Vec::new(),
            children: Vec::new(),
        })
    }
}

/// Command to modify a network
#[derive(serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum ModifyCmd<T, U> {
    /// Add a new bucket
    AddBucket {
        /// Parent path for the new bucket
        parent: Path,
    },
    /// Add a new joint
    AddJoint {
        /// Parent path for the new joint
        parent: Path,
    },
    /// Delete a node (bucket/joint) that is empty
    DeleteEmpty {
        /// Path of the node (bucket/joint) to delete
        path: Path,
    },
    /// Set the contents of the specified bucket
    ///
    /// Removes the bucket from the "needing fill" list (if present)
    FillBucket {
        /// Path of the bucket to fill
        bucket: Path,
        /// Items for the bucket
        new_contents: Vec<T>,
    },
    /// Set the filters on a joint
    SetJointFilters {
        /// Path for the existing joint
        joint: Path,
        /// List of filters to set on the joint
        new_filters: Vec<U>,
    },
}

/// Error modifying the [`Network`]
pub struct ModifyError(ModifyErr);
enum ModifyErr {
    UnknownPath(UnknownPath),
    AddToBucket(CannotAddToBucket),
    DeleteRoot,
    DeleteNonemptyBucket(CannotDeleteNonempty),
    DeleteNonemptyJoint(CannotDeleteNonempty),
    FilterRoot,
    FillJoint,
}
impl From<UnknownPath> for ModifyError {
    fn from(value: UnknownPath) -> Self {
        Self(ModifyErr::UnknownPath(value))
    }
}
impl From<CannotAddToBucket> for ModifyError {
    fn from(value: CannotAddToBucket) -> Self {
        Self(ModifyErr::AddToBucket(value))
    }
}
impl From<ModifyErr> for ModifyError {
    fn from(value: ModifyErr) -> Self {
        Self(value)
    }
}

impl std::error::Error for ModifyError {}
impl std::fmt::Display for ModifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self(inner) = self;
        match inner {
            ModifyErr::UnknownPath(UnknownPath(path)) => write!(f, "unknown path: {path:?}"),
            ModifyErr::AddToBucket(CannotAddToBucket(path)) => {
                write!(f, "cannot add to bucket: {path:?}")
            }
            ModifyErr::DeleteRoot => write!(f, "cannot delete the spigot (root node)"),
            ModifyErr::DeleteNonemptyBucket(CannotDeleteNonempty(path)) => {
                write!(f, "cannot delete non-empty bucket: {path:?}")
            }
            ModifyErr::DeleteNonemptyJoint(CannotDeleteNonempty(path)) => {
                write!(f, "cannot delete non-empty joint: {path:?}")
            }
            ModifyErr::FilterRoot => write!(f, "cannot filter the spigot (root node)"),
            ModifyErr::FillJoint => {
                write!(f, "cannot fill joint (only buckets have items)")
            }
        }
    }
}
impl std::fmt::Debug for ModifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ModifyError({self})")
    }
}

/// The specified path does not match a node (any of the joints, buckets, or root spigot)
#[derive(Debug)]
pub struct UnknownPath(Path);
/// Buckets cannot have filters or child joints or buckets
pub struct CannotAddToBucket(Path);
/// Only allowed to delete empty joints or buckets
pub struct CannotDeleteNonempty(Path);

#[cfg(test)]
#[allow(clippy::panic)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use arb_rng::{assert_arb_error, fake_rng, PanicRng};

    mod arb_rng {

        /// Random Number Generator that is fed by a deterministic `arbtest::arbitrary`
        pub(super) struct ArbitraryRng<'a, 'b>(&'a mut arbtest::arbitrary::Unstructured<'b>)
        where
            'b: 'a;
        impl<'a, 'b> rand::RngCore for ArbitraryRng<'a, 'b> {
            fn next_u32(&mut self) -> u32 {
                unimplemented!("non-fallible RngCore method called");
            }
            fn next_u64(&mut self) -> u64 {
                unimplemented!("non-fallible RngCore method called");
            }
            fn fill_bytes(&mut self, _dest: &mut [u8]) {
                unimplemented!("non-fallible RngCore method called");
            }
            fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand::Error> {
                for dest in dest {
                    *dest = self.0.arbitrary().map_err(rand::Error::new)?;
                }
                Ok(())
            }
        }
        pub(super) fn fake_rng<'a, 'b>(
            arbitrary: &'a mut arbtest::arbitrary::Unstructured<'b>,
        ) -> ArbitraryRng<'a, 'b> {
            ArbitraryRng(arbitrary)
        }

        /// Rng that panics when called
        pub(super) struct PanicRng;
        impl rand::RngCore for PanicRng {
            fn next_u32(&mut self) -> u32 {
                unreachable!("next_u32 in PanicRng");
            }
            fn next_u64(&mut self) -> u64 {
                unreachable!("next_u64 in PanicRng");
            }
            fn fill_bytes(&mut self, _dest: &mut [u8]) {
                unreachable!("fill_bytes in PanicRng");
            }
            fn try_fill_bytes(&mut self, _dest: &mut [u8]) -> Result<(), rand::Error> {
                unreachable!("try_fill_bytes in PanicRng");
            }
        }

        fn extract_arb_error<T>(
            inner_fn: impl FnOnce() -> Result<T, rand::Error>,
        ) -> Result<Result<T, arbtest::arbitrary::Error>, Box<dyn std::error::Error + Sync + Send>>
        {
            match inner_fn() {
                Ok(value) => Ok(Ok(value)),
                Err(err) => {
                    let inner_error = err.take_inner();
                    match inner_error.downcast() {
                        Ok(arb_error) => Ok(Err(*arb_error)),
                        Err(other_error) => Err(other_error),
                    }
                }
            }
        }
        pub(super) fn assert_arb_error<T>(
            inner_fn: impl FnOnce() -> Result<T, rand::Error>,
        ) -> Result<T, arbtest::arbitrary::Error> {
            extract_arb_error(inner_fn)
                .expect("expected only arbitrary::Error can be thrown by RNG")
        }
    }

    mod network_script {
        use crate::{
            clap::ModifyCmd as ClapModifyCmd, path::Path, ModifyCmd, ModifyError, Network,
        };
        use ::clap::Parser as _;

        #[derive(serde::Serialize)]
        pub(super) struct Log<U>(Vec<Entry<U>>);
        #[derive(Debug, serde::Serialize)]
        pub(super) enum Entry<U> {
            BucketsNeedingFill(Vec<Path>),
            Filters(Path, Vec<Vec<U>>),
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

        pub(super) struct Runner<T, U> {
            network: Network<T, U>,
        }
        impl<T, U> Network<T, U> {
            pub(super) fn into_runner(self) -> Runner<T, U> {
                Runner { network: self }
            }
        }
        impl Network<String, String> {
            pub(super) fn strs_runner() -> Runner<String, String> {
                Self::default().into_runner()
            }
        }

        impl<T, U> Runner<T, U>
        where
            T: crate::clap::ArgBounds,
            U: crate::clap::ArgBounds,
        {
            pub(super) fn into_inner(self) -> Network<T, U> {
                let Self { network } = self;
                network
            }
            pub(super) fn run_script(&mut self, commands: &'static str) -> Log<U> {
                let entries = commands
                    .lines()
                    .filter_map(|cmd| 'run_cmd: {
                        let cmd = cmd.trim();
                        if cmd.is_empty() || cmd.starts_with('#') {
                            break 'run_cmd None;
                        }
                        self.run_script_command(cmd)
                            .unwrap_or_else(|err| panic!("error running command {cmd:?}:\n{err}"))
                    })
                    .collect();
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
                        self.network.modify(cmd)?;

                        let entry = if output_buckets {
                            let buckets = self.network.get_buckets_needing_fill();
                            Some(Entry::BucketsNeedingFill(buckets.to_owned()))
                        } else {
                            None
                        };
                        Ok(entry)
                    }
                    Command::GetFilters { path } => {
                        let filters = self
                            .network
                            .get_filters(path.clone())
                            .map_err(ModifyError::from)?;
                        let filters = filters
                            .iter()
                            .map(|&filter_set| filter_set.to_owned())
                            .collect();
                        Ok(Some(Entry::Filters(path, filters)))
                    }
                }
            }
            pub(super) fn expect_command_error<V>(
                &mut self,
                (why_error, command_str): (&'static str, &'static str),
            ) -> V
            where
                V: std::error::Error + 'static,
            {
                let value = self
                    .run_script_command(command_str)
                    .expect_err(why_error)
                    .downcast::<V>()
                    .unwrap_or_else(|other_err| {
                        panic!(
                            "expected error to be of type {}: {other_err}",
                            std::any::type_name::<V>()
                        )
                    });
                // unbox
                *value
            }
        }
    }

    #[test]
    fn empty() {
        let network = Network::<(), ()>::default();
        arbtest::arbtest(|u| {
            let peeked = assert_arb_error(|| network.peek(&mut fake_rng(u), usize::MAX))?;
            assert_eq!(peeked, &[]);
            Ok(())
        });
    }

    #[test]
    fn joint_filters() {
        let log = Network::<u8, i32>::default().into_runner().run_script(
            "
            modify add-joint .
            modify set-joint-filters .0 1 2 3
            get-filters .0
            ",
        );
        insta::assert_ron_snapshot!(log, @r###"
        Log([
          Filters(".0", [
            [
              1,
              2,
              3,
            ],
          ]),
        ])
        "###);
    }

    #[test]
    fn single_bucket() {
        let network = Network::<String, u8>::default();
        let mut runner = network.into_runner();
        let log = runner.run_script(
            "
            modify add-bucket .
            modify fill-bucket .0 a b c
            ",
        );
        insta::assert_ron_snapshot!(log, @r###"
        Log([
          BucketsNeedingFill([
            ".0",
          ]),
          BucketsNeedingFill([]),
        ])
        "###);

        let network = runner.into_inner();
        let peeked = network.peek(&mut PanicRng, usize::MAX).unwrap();
        let empty: &[&str] = &[];
        assert_eq!(peeked, empty);
    }

    #[test]
    fn delete_empty_bucket() {
        let mut runner = Network::strs_runner();
        let log = runner.run_script(
            "
            modify add-bucket .
            modify fill-bucket .0 abc def
            ",
        );
        insta::assert_ron_snapshot!(log, @r###"
        Log([
          BucketsNeedingFill([
            ".0",
          ]),
          BucketsNeedingFill([]),
        ])
        "###);

        let result = runner.expect_command_error::<ModifyError>((
            "delete non-empty bucket",
            "modify delete-empty .0",
        ));
        insta::assert_snapshot!(result, @"cannot delete non-empty bucket: Path(.0)");

        let log = runner.run_script(
            "
            modify fill-bucket .0
            modify delete-empty .0
            ",
        );
        insta::assert_ron_snapshot!(log, @r###"
        Log([
          BucketsNeedingFill([]),
        ])
        "###);
    }
    #[test]
    fn delete_empty_joint() {
        let mut runner = Network::strs_runner();
        let log = runner.run_script(
            "
            modify add-joint .
            modify add-joint .0
            ",
        );
        insta::assert_ron_snapshot!(log, @"Log([])");

        let result = runner.expect_command_error::<ModifyError>((
            "delete non-empty joint",
            "modify delete-empty .0",
        ));
        insta::assert_snapshot!(result, @"cannot delete non-empty joint: Path(.0)");

        let log = runner.run_script(
            "
            modify delete-empty .0.0
            modify delete-empty .0
            ",
        );
        insta::assert_ron_snapshot!(log, @"Log([])");
    }
}
