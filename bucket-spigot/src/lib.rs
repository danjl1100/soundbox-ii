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

use child_vec::{ChildVec, Weights};
use path::Path;
use std::collections::HashSet;

mod child_vec;
pub mod clap;
pub mod path;
mod ser;

pub mod order {
    //! Ordering for selecting child nodes and child items throughout the
    //! [`Network`](`crate::Network`)

    type RandResult<T> = Result<T, rand::Error>;

    use counts_remaining::CountsRemaining;
    pub(crate) use node::Node as OrderNode;
    pub(crate) use node::{Root, UnknownOrderPath};
    pub use peek::Peeked;
    use source::Order;
    #[allow(clippy::module_name_repetitions)]
    pub use source::OrderType;

    mod counts_remaining;
    mod node;
    mod peek;
    mod source;

    #[cfg(test)]
    mod tests;
}

pub mod view {
    //! Views for a [`Network`](`crate::Network`)

    use table_model::NodeKind;
    #[allow(clippy::module_name_repetitions)]
    pub use table_model::TableView;
    pub use table_model::{Cell, NodeDetails, Row};
    mod table_model;

    pub use table::{TableParams, TableParamsOwned};
    mod table;

    mod error;
}

/// Group of buckets with a central spigot
#[derive(Clone, Debug)]
pub struct Network<T, U> {
    root: ChildVec<Child<T, U>>,
    buckets_needing_fill: HashSet<Path>,
    /// Order stored separately for ease of mutation/cloning in [`Self::peek`]
    root_order: order::Root,
    bucket_id_counter: u64,
}
impl<T, U> Default for Network<T, U> {
    fn default() -> Self {
        Self {
            root: ChildVec::default(),
            buckets_needing_fill: HashSet::default(),
            root_order: order::Root::default(),
            bucket_id_counter: 0,
        }
    }
}

type OptChildrenRef<'a, T, U> = Option<&'a ChildVec<Child<T, U>>>;
type OptChildRef<'a, T, U> = Option<&'a Child<T, U>>;
type OptChildrenAndChildRef<'a, T, U> = (OptChildrenRef<'a, T, U>, OptChildRef<'a, T, U>);

impl<T, U> Network<T, U> {
    /// Modify the network topology
    ///
    /// # Errors
    /// Returns an error if the command does not match the current network state
    pub fn modify(&mut self, cmd: ModifyCmd<T, U>) -> Result<(), ModifyError> {
        match cmd {
            ModifyCmd::AddBucket { parent } => {
                let bucket = Child::Bucket(self.new_bucket());
                let _path = self.add_child(bucket, parent)?;
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
            ModifyCmd::SetOrderType {
                path,
                new_order_type,
            } => Ok(self
                .root_order
                .set_order_type(new_order_type, path.as_ref())?),
        }
    }
    fn new_bucket(&mut self) -> Bucket<T, U> {
        let id = self.bucket_id_counter;
        self.bucket_id_counter += 1;
        Bucket::new(BucketId(id))
    }
    // /// Returns the [`Path`] to the specified [`BucketId`], if any exists
    // pub fn find_bucket_path(&self, id: BucketId) -> Option<Path> { // TODO
    //     todo!()
    // }
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
    pub fn get_filters(&self, path: path::PathRef<'_>) -> Result<Vec<&[U]>, UnknownPath> {
        let mut filter_groups = Vec::new();

        self.for_each_child(path, |child| {
            let filters = match child {
                Child::Bucket(bucket) => &bucket.filters,
                Child::Joint(joint) => &joint.filters,
            };
            if !filters.is_empty() {
                filter_groups.push(&filters[..]);
            }
        })
        .map_err(UnknownPathRef::to_owned)?;

        Ok(filter_groups)
    }

    /// Returns the children at the path (if any) and the matched node (if not root)
    fn for_each_child<'a, 'b>(
        &'a self,
        path: path::PathRef<'b>,
        mut process_child_fn: impl FnMut(&'a Child<T, U>),
    ) -> Result<OptChildrenAndChildRef<'a, T, U>, UnknownPathRef<'b>> {
        let mut current = Some(&self.root);
        let mut found = None;

        for next_index in path {
            let Some(next_child) = current.and_then(|c| c.children().get(next_index)) else {
                return Err(UnknownPathRef(path));
            };

            process_child_fn(next_child);
            found = Some(next_child);

            current = match next_child {
                Child::Bucket(_) => None,
                Child::Joint(joint) => Some(&joint.next),
            };
        }

        Ok((current, found))
    }

    #[cfg(test)]
    fn count_child_nodes_of<'a>(
        &self,
        path: path::PathRef<'a>,
    ) -> Result<Option<usize>, UnknownPathRef<'a>> {
        let (children, _found) = self.for_each_child(path, |_| {})?;

        let child_node_count = children.map(child_vec::ChildVec::len);
        Ok(child_node_count)
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
    id: BucketId,
}
#[derive(Clone, Debug)]
struct Joint<T, U> {
    next: ChildVec<Child<T, U>>,
    filters: Vec<U>,
}

/// Identifier for a specific bucket
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BucketId(pub u64);

impl<T, U> Bucket<T, U> {
    fn new(id: BucketId) -> Self {
        Self {
            items: vec![],
            filters: vec![],
            id,
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

/// Command to modify a network
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
    /// Set the ordering type for the joint or bucket
    SetOrderType {
        /// Path for the existing joint or bucket
        path: Path,
        /// Order type (how to select from immediate child nodes or items)
        new_order_type: order::OrderType,
    },
    // TODO MoveBucket
    // TODO MoveJoint (unless this destroys the BucketId -> Path logic) MoveEmptyJoint?
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
            ModifyErr::UnknownOrderPath(err) => {
                write!(f, "{err}")
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
impl UnknownPath {
    /// Returns an error with a reference to the inner [`Path`]
    #[must_use]
    pub fn as_ref(&self) -> UnknownPathRef<'_> {
        UnknownPathRef(self.0.as_ref())
    }
}
impl std::fmt::Display for UnknownPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_ref().fmt(f)
    }
}

/// The specified path does not match a node (any of the joints, buckets, or root spigot)
#[derive(Clone, Copy, Debug)]
pub struct UnknownPathRef<'a>(path::PathRef<'a>);
impl UnknownPathRef<'_> {
    /// Clones to create an owned version of the error
    fn to_owned(self) -> UnknownPath {
        UnknownPath(self.0.to_owned())
    }
}
impl std::fmt::Display for UnknownPathRef<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self(path) = self;
        write!(f, "unknown path: {path}")
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
    pub(crate) use arb_rng::{assert_arb_error, decode_hex, fake_rng, PanicRng};
    pub(crate) use sync::run_with_timeout;

    // utils
    mod arb_network;
    mod arb_rng;
    mod script;
    mod sync;

    // test cases
    mod clap;
    mod modify_network;
    mod peek_effort;
    mod peek_pop_network;
    mod ser;
    mod view_table;
}
