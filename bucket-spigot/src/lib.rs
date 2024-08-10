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
//! Modifying *joint filters* queues downstream buckets to be *refilled*.
//! The user provides a list of items to fill each bucket based on the sequence of filters passed when
//! walking from the spigot (root node) to the bucket.
//!

use child_vec::ChildVec;
use path::Path;
use std::collections::HashSet;

pub mod clap;
pub mod path;

pub mod order;

/// Group of buckets with a central spigot
#[derive(Clone, Debug, Default)]
pub struct Network<T, U> {
    root: ChildVec<Child<T, U>>,
    buckets_needing_fill: HashSet<Path>,
    /// Order stored separately for ease of mutation/cloning in [`Self::peek`]
    root_order: order::Root,
}
impl<T, U> Network<T, U> {
    /// Modify the network topology
    ///
    /// # Errors
    /// Returns an error if the command does not match the current network state
    pub fn modify(&mut self, cmd: ModifyCmd<T, U>) -> Result<(), ModifyError> {
        match cmd {
            ModifyCmd::AddBucket { parent } => {
                let _path = self.add_child(Child::Bucket(Bucket::default()), parent)?;
                Ok(())
            }
            ModifyCmd::AddJoint { parent } => {
                let _path = self.add_child(Child::Joint(Joint::default()), parent)?;
                Ok(())
            }
            ModifyCmd::DeleteEmpty { path } => self.delete_empty(path),
            ModifyCmd::FillBucket {
                bucket,
                new_contents,
            } => self.set_bucket_items(new_contents, bucket),
            ModifyCmd::SetFilters { path, new_filters } => self.set_filters(new_filters, path),
            ModifyCmd::SetWeight { path, new_weight } => self.set_weight(new_weight, path),
        }
    }
    /// Returns the paths to buckets needing to be filled (e.g. filters may have changed)
    pub fn get_buckets_needing_fill(&self) -> impl Iterator<Item = &'_ Path> {
        self.buckets_needing_fill.iter()
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

        let mut current = Some(&self.root);

        // TODO [1/5] find common pattern to simplify similar indexing logic...?
        for next_index in &path {
            let Some(next_child) = current.and_then(|c| c.children().get(next_index)) else {
                return Err(UnknownPath(path));
            };

            let filters = match next_child {
                Child::Bucket(bucket) => &bucket.filters,
                Child::Joint(joint) => &joint.filters,
            };
            if !filters.is_empty() {
                filter_groups.push(&filters[..]);
            }

            current = match next_child {
                Child::Bucket(_) => None,
                Child::Joint(joint) => Some(&joint.next),
            };
        }

        Ok(filter_groups)
    }

    fn add_child(&mut self, child: Child<T, U>, parent_path: Path) -> Result<Path, ModifyError> {
        let mut current = &mut self.root;

        // TODO [2/5] find common pattern to simplify similar indexing logic...?
        for next_index in &parent_path {
            let Some(next_child) = current.children_mut().get_mut(next_index) else {
                return Err(UnknownPath(parent_path).into());
            };
            current = match next_child {
                Child::Bucket(_) => {
                    return Err(CannotAddToBucket(parent_path).into());
                }
                Child::Joint(joint) => &mut joint.next,
            };
        }

        // add order for child (fails if node/order structures are not identical)
        let child_index = self.root_order.add(parent_path.as_ref())?;

        let child_index_expected = current.len();
        assert_eq!(
            child_index, child_index_expected,
            "order nodes should match item nodes"
        );

        let is_bucket = matches!(child, Child::Bucket(_));

        // add child
        current.push(child);

        // build child path
        let child_path = {
            let mut path = parent_path;
            path.push(child_index);
            path
        };

        // queue for refilling new bucket
        if is_bucket {
            self.buckets_needing_fill.insert(child_path.clone());
        }

        Ok(child_path)
    }
    fn delete_empty(&mut self, path: Path) -> Result<(), ModifyError> {
        let mut current = &mut self.root;

        let Some((final_index, parent_path)) = path.as_ref().split_last() else {
            return Err(ModifyErr::DeleteRoot.into());
        };

        // TODO [3/5] find common pattern to simplify similar indexing logic...?
        for next_index in parent_path {
            let Some(next_child) = current.children_mut().get_mut(next_index) else {
                return Err(UnknownPath(path).into());
            };
            current = match next_child {
                Child::Bucket(_) => {
                    return Err(UnknownPath(path).into());
                }
                Child::Joint(joint) => &mut joint.next,
            };
        }

        let Some(target_elem) = current.children().get(final_index) else {
            return Err(UnknownPath(path).into());
        };

        match target_elem {
            Child::Bucket(bucket) if !bucket.items.is_empty() => {
                Err(ModifyErr::DeleteNonemptyBucket(CannotDeleteNonempty(path)).into())
            }
            Child::Joint(joint) if !joint.next.is_empty() => {
                Err(ModifyErr::DeleteNonemptyJoint(CannotDeleteNonempty(path)).into())
            }
            Child::Bucket(_) | Child::Joint(_) => {
                current.remove(final_index);
                Ok(())
            }
        }
    }
    fn set_bucket_items(
        &mut self,
        new_contents: Vec<T>,
        bucket_path: Path,
    ) -> Result<(), ModifyError> {
        let mut current = &mut self.root;

        let mut bucket_path_iter = bucket_path.iter();
        let dest_contents = loop {
            let Some(next_index) = bucket_path_iter.next() else {
                return Err(ModifyErr::FillJoint.into());
            };
            let Some(next_child) = current.children_mut().get_mut(next_index) else {
                return Err(UnknownPath(bucket_path).into());
            };
            current = match next_child {
                Child::Bucket(bucket) => break bucket,
                Child::Joint(joint) => &mut joint.next,
            };
        };
        if bucket_path_iter.next().is_some() {
            return Err(UnknownPath(bucket_path).into());
        }

        self.buckets_needing_fill.remove(&bucket_path);

        dest_contents.items = new_contents;
        Ok(())
    }
    fn set_filters(&mut self, new_filters: Vec<U>, path: Path) -> Result<(), ModifyError> {
        let mut current = Some(&mut self.root);
        let mut dest_filters = None;
        // TODO [4/5] find common pattern to simplify similar indexing logic...?
        for next_index in &path {
            let Some(next_child) = current.and_then(|c| c.children_mut().get_mut(next_index))
            else {
                return Err(UnknownPath(path).into());
            };
            current = match next_child {
                Child::Bucket(bucket) => {
                    dest_filters = Some(&mut bucket.filters);
                    None
                }
                Child::Joint(joint) => {
                    dest_filters = Some(&mut joint.filters);
                    Some(&mut joint.next)
                }
            };
        }

        if let Some(dest_filters) = dest_filters {
            *dest_filters = new_filters;

            if let Some(joint_children) = current {
                // target is joint, search for all child buckets

                let mut joint_path_buf = path;
                Self::add_buckets_need_fill_under(
                    &mut joint_path_buf,
                    &mut self.buckets_needing_fill,
                    joint_children.children(),
                );
            } else {
                // target is bucket
                self.buckets_needing_fill.insert(path);
            }

            Ok(())
        } else {
            Err(ModifyErr::FilterRoot)?
        }
    }
    fn set_weight(&mut self, new_weight: u32, path: Path) -> Result<(), ModifyError> {
        let mut current = &mut self.root;
        let Some((last_index, parent_path)) = path.as_ref().split_last() else {
            return Err(ModifyErr::WeightRoot.into());
        };
        // TODO [5/5] find common pattern to simplify similar indexing logic...?
        for next_index in parent_path {
            let next_child = current.children_mut().get_mut(next_index);
            current = match next_child {
                None | Some(Child::Bucket(_)) => {
                    return Err(UnknownPath(path).into());
                }
                Some(Child::Joint(joint)) => &mut joint.next,
            };
        }

        let target_parent = current;
        if last_index < target_parent.len() {
            target_parent.set_weight(last_index, new_weight);
            Ok(())
        } else {
            Err(UnknownPath(path).into())
        }
    }
    fn add_buckets_need_fill_under(
        path: &mut Path,
        buckets_needing_fill: &mut HashSet<Path>,
        child_nodes: &[Child<T, U>],
    ) {
        for (index, child) in child_nodes.iter().enumerate() {
            path.push(index);
            match child {
                Child::Bucket(_) => {
                    buckets_needing_fill.insert(path.clone());
                }
                Child::Joint(joint) => {
                    Self::add_buckets_need_fill_under(
                        path,
                        buckets_needing_fill,
                        joint.next.children(),
                    );
                }
            }
            assert_eq!(path.pop(), Some(index), "should contain the pushed index");
        }
    }
}

#[derive(Clone, Debug)]
enum Child<T, U> {
    Bucket(Bucket<T, U>),
    Joint(Joint<T, U>),
}
#[derive(Clone, Debug)]
struct Bucket<T, U> {
    items: Vec<T>,
    filters: Vec<U>,
}
#[derive(Clone, Debug)]
struct Joint<T, U> {
    next: ChildVec<Child<T, U>>,
    filters: Vec<U>,
}

impl<T, U> Default for Bucket<T, U> {
    fn default() -> Self {
        Self {
            items: vec![],
            filters: vec![],
        }
    }
}
impl<T, U> Default for Joint<T, U> {
    fn default() -> Self {
        Self {
            next: ChildVec::default(),
            filters: vec![],
        }
    }
}

mod child_vec {
    #[derive(Clone, Debug)]
    pub(crate) struct ChildVec<T> {
        children: Vec<T>,
        /// Weights for each child (may be empty if all are weighted equally)
        weights: Vec<u32>,
    }
    impl<T> From<Vec<T>> for ChildVec<T> {
        fn from(children: Vec<T>) -> Self {
            Self {
                children,
                weights: vec![],
            }
        }
    }
    impl<T> Default for ChildVec<T> {
        fn default() -> Self {
            vec![].into()
        }
    }
    impl<T> ChildVec<T> {
        pub fn children(&self) -> &[T] {
            &self.children
        }
        pub fn weights(&self) -> &[u32] {
            &self.weights
        }
        pub fn children_mut(&mut self) -> &mut [T] {
            &mut self.children
        }
        pub fn set_weight(&mut self, index: usize, value: u32) {
            if self.weights.is_empty() {
                self.weights = vec![1; self.len()];
            }
            self.weights[index] = value;
        }
        pub fn len(&self) -> usize {
            self.children.len()
        }
        pub fn is_empty(&self) -> bool {
            self.children.is_empty()
        }
        pub fn push(&mut self, child: T) {
            // update to unity weight (if needed)
            if !self.weights.is_empty() {
                self.weights.push(1);
            }

            self.children.push(child);
        }
        pub fn remove(&mut self, index: usize) -> (u32, T) {
            let child = self.children.remove(index);

            let weight = if self.weights.is_empty() {
                1
            } else {
                self.weights.remove(index)
            };
            (weight, child)
        }
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
    /// Set the filters on a joint or bucket
    SetFilters {
        /// Path for the existing joint or bucket
        path: Path,
        /// List of filters to set
        new_filters: Vec<U>,
    },
    /// Set the weight on a joint or bucket
    SetWeight {
        /// Path for the existing joint or bucket
        path: Path,
        /// Weight value (relative to other weights on sibling nodes)
        new_weight: u32,
    },
}

/// Error modifying the [`Network`]
pub struct ModifyError(ModifyErr);
enum ModifyErr {
    UnknownPath(UnknownPath),
    UnknownOrderPath(order::UnknownOrderPath),
    AddToBucket(CannotAddToBucket),
    DeleteRoot,
    DeleteNonemptyBucket(CannotDeleteNonempty),
    DeleteNonemptyJoint(CannotDeleteNonempty),
    FilterRoot,
    FillJoint,
    WeightRoot,
}
impl From<UnknownPath> for ModifyError {
    fn from(value: UnknownPath) -> Self {
        Self(ModifyErr::UnknownPath(value))
    }
}
impl From<order::UnknownOrderPath> for ModifyError {
    fn from(value: order::UnknownOrderPath) -> Self {
        Self(ModifyErr::UnknownOrderPath(value))
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
            ModifyErr::UnknownPath(err) => write!(f, "{err}"),
            ModifyErr::UnknownOrderPath(order::UnknownOrderPath(path)) => {
                write!(f, "unknown order path: {path:?}")
            }
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
            ModifyErr::WeightRoot => write!(f, "cannot weight the spigot (root node)"),
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
impl std::fmt::Display for UnknownPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self(path) = self;
        write!(f, "unknown path: {path:?}")
    }
}

/// Buckets cannot have filters or child joints or buckets
pub(crate) struct CannotAddToBucket(Path);
/// Only allowed to delete empty joints or buckets
pub(crate) struct CannotDeleteNonempty(Path);

#[cfg(test)]
#[allow(clippy::panic)]
#[allow(clippy::unwrap_used)]
mod tests {
    pub(crate) use arb_rng::{fake_rng, PanicRng};
    pub(crate) use sync::run_with_timeout;

    // utils
    mod arb_rng;
    mod script;
    mod sync;

    // test cases
    mod modify_network;
    mod peek_effort;
    mod peek_pop_network;
}
