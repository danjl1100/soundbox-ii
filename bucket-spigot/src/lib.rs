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

#![allow(unused)] // TODO only while building

// mod clap;
mod path;

/// Group of buckets with a central spigot
#[derive(Clone, Debug, Default)]
pub struct Network<T, U> {
    root: Option<Vec<Child<T, U>>>,
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
                self.push(Child::bucket(), parent)?;
                Ok(())
            }
            ModifyCmd::AddJoint { parent } => {
                self.push(Child::joint(), parent)?;
                Ok(())
            }
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

        let mut current_children = self.root.as_ref();

        for &next_index in &path {
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

    fn push(&mut self, child: Child<T, U>, parent_path: Path) -> Result<Path, ModifyError> {
        let mut current_children = if let Some(root) = &mut self.root {
            root
        } else {
            self.root = Some(Vec::new());
            self.root.as_mut().expect("initialized root just now")
        };

        for &next_index in &parent_path {
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
    fn set_bucket_items(
        &mut self,
        new_contents: Vec<T>,
        bucket_path: Path,
    ) -> Result<(), ModifyError> {
        let mut current_children = self.root.as_mut();

        let mut bucket_path_iter = bucket_path.iter();
        let dest_contents = loop {
            let Some(&next_index) = bucket_path_iter.next() else {
                return Err(ModifyErr::CannotFillJoint.into());
            };
            let Some(next_child) = current_children.and_then(|c| c.get_mut(next_index)) else {
                return Err(UnknownPath(bucket_path).into());
            };
            current_children = match next_child {
                Child::Bucket(bucket) => break bucket,
                Child::Joint(joint) => Some(&mut joint.children),
            };
        };

        *dest_contents = new_contents;
        Ok(())
    }
    fn set_joint_filters(
        &mut self,
        new_filters: Vec<U>,
        joint_path: Path,
    ) -> Result<(), ModifyError> {
        let mut current_children = self.root.as_mut();
        let mut dest_filters = None;
        for &next_index in &joint_path {
            let Some(next_child) = current_children.and_then(|c| c.get_mut(next_index)) else {
                return Err(UnknownPath(joint_path).into());
            };
            current_children = match next_child {
                Child::Bucket(_) => {
                    return Err(CannotAddToBucket(joint_path).into());
                }
                Child::Joint(joint) => {
                    dest_filters = Some(&mut joint.filters);
                    Some(&mut joint.children)
                }
            };
        }

        if let Some(dest_filters) = dest_filters {
            *dest_filters = new_filters;
            Ok(())
        } else {
            Err(ModifyErr::CannotFilterRoot)?
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

// TODO
type Path = Vec<usize>;
// type PathRef<'a> = &'a [usize];

/// Command to modify a network
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
        /// List of filters to set on the joint
        new_filters: Vec<U>,
        /// Path for the existing joint
        joint: Path,
    },
}

/// Error modifying the [`Network`]
pub struct ModifyError(ModifyErr);
enum ModifyErr {
    UnknownPath(UnknownPath),
    CannotAddToBucket(CannotAddToBucket),
    CannotFilterRoot,
    CannotFillJoint,
}
impl From<UnknownPath> for ModifyError {
    fn from(value: UnknownPath) -> Self {
        Self(ModifyErr::UnknownPath(value))
    }
}
impl From<CannotAddToBucket> for ModifyError {
    fn from(value: CannotAddToBucket) -> Self {
        Self(ModifyErr::CannotAddToBucket(value))
    }
}
impl From<ModifyErr> for ModifyError {
    fn from(value: ModifyErr) -> Self {
        Self(value)
    }
}

impl std::fmt::Display for ModifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self(inner) = self;
        match inner {
            ModifyErr::UnknownPath(UnknownPath(path)) => write!(f, "unknown path: {path:?}"),
            ModifyErr::CannotAddToBucket(CannotAddToBucket(path)) => {
                write!(f, "cannot add to bucket: {path:?}")
            }
            ModifyErr::CannotFilterRoot => write!(f, "cannot filter the spigot (root node)"),
            ModifyErr::CannotFillJoint => {
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

#[cfg(test)]
#[allow(clippy::panic)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::any::Any;

    /// Random Number Generator that is fed by a deterministic `arbtest::arbitrary`
    struct ArbitraryRng<'a, 'b>(&'a mut arbtest::arbitrary::Unstructured<'b>)
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
    fn fake_rng<'a, 'b>(
        arbitrary: &'a mut arbtest::arbitrary::Unstructured<'b>,
    ) -> ArbitraryRng<'a, 'b> {
        ArbitraryRng(arbitrary)
    }

    /// Rng that panics when called
    struct PanicRng;
    impl rand::RngCore for PanicRng {
        fn next_u32(&mut self) -> u32 {
            unreachable!("next_u32 in PanicRng");
        }
        fn next_u64(&mut self) -> u64 {
            unreachable!("next_u64 in PanicRng");
        }
        fn fill_bytes(&mut self, dest: &mut [u8]) {
            unreachable!("fill_bytes in PanicRng");
        }
        fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand::Error> {
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
    fn assert_arb_error<T>(
        inner_fn: impl FnOnce() -> Result<T, rand::Error>,
    ) -> Result<T, arbtest::arbitrary::Error> {
        extract_arb_error(inner_fn).expect("expected only arbitrary::Error can be thrown by RNG")
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
        let mut network = Network::<(), _>::default();
        network
            .modify(ModifyCmd::AddJoint { parent: vec![] })
            .unwrap();
        let joint1 = vec![0];
        network
            .modify(ModifyCmd::SetJointFilters {
                new_filters: vec![1, 2, 3],
                joint: joint1.clone(),
            })
            .unwrap();
        insta::assert_ron_snapshot!(network.get_filters(joint1).unwrap(), @r###"
        [
          [
            1,
            2,
            3,
          ],
        ]
        "###);
    }

    #[test]
    fn single_bucket() {
        let mut network = Network::<_, ()>::default();
        network
            .modify(ModifyCmd::AddBucket { parent: vec![] })
            .unwrap();

        let paths = network.get_buckets_needing_fill();
        insta::assert_ron_snapshot!(paths, @r###"
        [
          [
            0,
          ],
        ]
        "###);
        let Some((bucket, &[])) = paths.split_first() else {
            panic!("expected one path")
        };

        network
            .modify(ModifyCmd::FillBucket {
                bucket: bucket.to_owned(),
                new_contents: vec!["a", "b", "c"],
            })
            .unwrap();

        let peeked = network.peek(&mut PanicRng, usize::MAX).unwrap();
        let empty: &[&str] = &[];
        assert_eq!(peeked, empty);
    }
}
