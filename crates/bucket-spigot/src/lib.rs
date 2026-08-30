// soundbox-ii/filter-buckets Item accumulations for sequencing *don't keep your sounds boxed up*
// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

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

use crate::{
    order::UnknownOrderPath,
    path::{Path, PathNonempty, PathSlice, PathSliceNonempty},
    traversal::ChildFound,
};
// TODO - extract modules from this (long) lib.rs

use bucket_paths_map::BucketPathsMap;
use child_vec::{ChildVec, Weights};

mod child_vec;
pub mod clap;
pub mod path;
mod ser;
mod traversal;

pub mod order {
    //! Ordering for selecting child nodes and child items throughout the
    //! [`Network`](`crate::Network`)

    use self::counts_remaining::CountsRemaining;
    pub use self::fallible_rng::{ArbitrarySource, ErrorRng};
    pub(crate) use self::node::Node as OrderNode;
    pub(crate) use self::node::{Root, UnknownOrderPath};
    pub use self::peek::{PeekAccepted, Peeked};
    use self::source::Order;
    #[expect(clippy::module_name_repetitions, reason = "name for re-export")]
    pub use self::source::OrderType;

    mod counts_remaining;
    mod fallible_rng;
    mod node;
    mod peek;
    mod source;

    #[cfg(test)]
    mod tests;
}

pub mod view {
    //! Views for a [`Network`](`crate::Network`)

    use table_model::NodeKind;
    #[expect(clippy::module_name_repetitions, reason = "name for re-export")]
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
    trees: Trees<T, U>,
    bucket_paths: BucketPathsMap,
    bucket_id_counter: u64,
}
/// Node-tree portions of a network
#[derive(Clone, Debug)]
struct Trees<T, U> {
    /// Nodes containing the joints/buckets and items
    item: ChildVec<Child<T, U>>,
    /// Order stored separately for ease of mutation/cloning in [`Network::peek`]
    order: order::Root,
}
impl<T, U> Default for Network<T, U> {
    fn default() -> Self {
        Self {
            trees: Trees {
                item: ChildVec::default(),
                order: order::Root::default(),
            },
            bucket_paths: BucketPathsMap::default(),
            bucket_id_counter: 0,
        }
    }
}

impl<T, U> Network<T, U> {
    /// Modify the network topology
    ///
    /// # Errors
    /// Returns an error if the command does not match the current network state
    pub fn modify(&mut self, cmd: ModifyCmd<T, U>) -> Result<(), ModifyError> {
        self.modify_and_get_created_path(cmd).map(|_| ())
    }
    /// Modify the network topology, returning a [`Path`] if a bucket or joint
    /// was created
    ///
    /// # Errors
    /// Returns an error if the command does not match the current network state
    pub fn modify_and_get_created_path(
        &mut self,
        cmd: ModifyCmd<T, U>,
    ) -> Result<Option<Path>, ModifyError> {
        let result = match cmd {
            ModifyCmd::AddBucket { new_path } => {
                let bucket = Child::Bucket(self.new_bucket());
                self.add_child_at(bucket, new_path)?;
                Ok(None)
            }
            ModifyCmd::AddBucketTo { parent } => {
                let bucket = Child::Bucket(self.new_bucket());
                let path = self.add_child_under(bucket, parent)?;
                Ok(Some(path))
            }
            ModifyCmd::AddJoint { new_path } => {
                self.add_child_at(Child::Joint(Joint::default()), new_path)?;
                Ok(None)
            }
            ModifyCmd::AddJointTo { parent } => {
                let path = self.add_child_under(Child::Joint(Joint::default()), parent)?;
                Ok(Some(path))
            }
            ModifyCmd::DeleteEmpty { path } => self.delete_empty(path).map(|()| None),
            ModifyCmd::FillBucket {
                bucket,
                new_contents,
            } => self.set_bucket_items(new_contents, &bucket).map(|()| None),
            ModifyCmd::SetFilters { path, new_filters } => {
                self.set_filters(new_filters, path).map(|()| None)
            }
            ModifyCmd::SetWeight { path, new_weight } => {
                self.set_weight(new_weight, path).map(|()| None)
            }
            ModifyCmd::SetOrderType {
                path,
                new_order_type,
            } => Ok(self
                .trees
                .order
                .set_order_type(new_order_type, &path)
                .map_err(ModifyErr::from)?)
            .map(|()| None),
        };

        #[cfg(test)]
        self.trees.assert_topologies_match();

        result.map_err(ModifyError::from)
    }
    fn new_bucket(&mut self) -> Bucket<T, U> {
        let id = self.bucket_id_counter;
        self.bucket_id_counter += 1;
        Bucket::new(BucketId(id))
    }
    /// Returns the [`Path`] to the specified [`BucketId`], if any exists
    ///
    /// # Errors
    /// Returns an error if the bucket id does not match any live bucket nodes
    pub fn find_bucket_path(&mut self, id: BucketId) -> Result<&PathSlice, UnknownBucketId> {
        self.bucket_paths.get_cached(id).ok_or(UnknownBucketId(id))
    }
    /// Returns the paths to buckets needing to be filled (e.g. filters may have changed)
    pub fn get_buckets_needing_fill(&mut self) -> impl Iterator<Item = &PathSlice> {
        // pre-populate cache
        if self.bucket_paths.is_cache_missing_any_need_fill() {
            // effort to cache 1 item is not significantly different from refreshing entire cache
            self.trees.visit_depth_first(|elem| {
                if let Child::Bucket(bucket) = elem.node_item {
                    self.bucket_paths.add_cached(bucket.id, elem.node_path);
                }
            });
        }

        self.bucket_paths.iter_needs_fill().map(|id| {
            self.bucket_paths.get_cached(id).unwrap_or_else(|| {
                unreachable!("bucket ids needing fill should be in cache from tree traversal");
            })
        })
    }
    /// Returns the filters for the specified path
    ///
    /// NOTE: Returns an empty set for the root path, as the spigot has no filters
    ///
    /// # Errors
    ///
    /// Returns an error if the path is unknown
    pub fn get_filters<'a>(&self, path: &'a PathSlice) -> Result<Vec<&[U]>, &'a UnknownPathSlice> {
        let mut filter_groups = Vec::new();

        self.trees.item.for_each_direct_child(path, |child| {
            let filters = match child {
                Child::Bucket(bucket) => &bucket.filters,
                Child::Joint(joint) => &joint.filters,
            };
            if !filters.is_empty() {
                filter_groups.push(&filters[..]);
            }
        })?;

        Ok(filter_groups)
    }

    #[cfg(test)]
    /// Counts the direct children of the specified joint node (`None` for bucket)
    ///
    /// # Errors
    /// Returns an error if the specified path is invalid
    fn count_direct_child_nodes_of<'a>(
        &self,
        path: &'a PathSlice,
    ) -> Result<Option<usize>, &'a UnknownPathSlice> {
        let (children, _found) = self.trees.item.for_each_direct_child(path, |_| {})?;

        let child_node_count = children.map(child_vec::ChildVec::len);
        Ok(child_node_count)
    }
    #[cfg(test)]
    /// Counts the all nodes in the network
    fn count_all_nodes(&self) -> usize {
        let mut total_count = 0;
        self.trees.visit_depth_first_items(|_| {
            total_count += 1;
        });

        total_count
    }

    fn add_child_at(&mut self, child: Child<T, U>, new_path: Path) -> Result<(), ModifyErr> {
        let Some(new_path_ref) = PathSliceNonempty::try_new(&new_path) else {
            return Err(ModifyErr::AddAtPath(new_path));
        };
        let (child_index, parent_path) = new_path_ref.split_last();

        let dest = self.trees.item.find_child_mut(parent_path);
        let dest = match dest {
            Ok(ChildFound::RootChildren(child_vec)) => child_vec,
            Ok(ChildFound::Joint(joint)) => &mut joint.next,
            Ok(ChildFound::Bucket(_)) => {
                return Err(CannotAddToBucket(parent_path.to_owned()).into());
            }
            Err(UnknownPathSlice(_)) => return Err(UnknownPath(parent_path.to_owned()).into()),
        };

        if dest.len() != child_index {
            return Err(ModifyErr::AddAtPath(new_path));
        }

        Self::insert_node(
            child,
            new_path_ref,
            &mut self.trees.order,
            &mut self.bucket_paths,
            dest,
        )?;

        Ok(())
    }

    fn add_child_under(
        &mut self,
        child: Child<T, U>,
        parent_path: Path,
    ) -> Result<Path, ModifyErr> {
        let dest = self.trees.item.find_child_mut(&parent_path);
        let dest = match dest {
            Ok(ChildFound::RootChildren(child_vec)) => child_vec,
            Ok(ChildFound::Joint(joint)) => &mut joint.next,
            Ok(ChildFound::Bucket(_)) => return Err(CannotAddToBucket(parent_path).into()),
            Err(UnknownPathSlice(_)) => return Err(UnknownPath(parent_path).into()),
        };

        // build child path
        let child_path = {
            let child_index = dest.len();
            PathNonempty::new(parent_path, child_index)
        };

        Self::insert_node(
            child,
            child_path.as_ref(),
            &mut self.trees.order,
            &mut self.bucket_paths,
            dest,
        )?;

        Ok(child_path.into_inner())
    }

    /// Adds an order node (panics if the trees are out of sync), adds the
    /// child to `dest`, and if the node is a bucket then queues for fill
    #[track_caller]
    fn insert_node(
        child: Child<T, U>,
        child_path: &PathSliceNonempty,
        trees_order: &mut order::Root,
        bucket_paths: &mut BucketPathsMap,
        dest: &mut ChildVec<Child<T, U>>,
    ) -> Result<(), UnknownOrderPath> {
        let (child_index, parent_path) = child_path.split_last();
        assert_eq!(
            child_index,
            dest.len(),
            "insert_node child_path index should match destination"
        );

        let bucket_id = if let Child::Bucket(bucket) = &child {
            Some(bucket.id)
        } else {
            None
        };

        // add order for child (fails if node/order structures are not identical)
        let child_index_order = trees_order.add(parent_path)?;
        assert_eq!(
            child_index_order, child_index,
            "order nodes should match item nodes"
        );

        // add child
        dest.push(child);

        // queue for refilling new bucket
        if let Some(bucket_id) = bucket_id {
            bucket_paths.add_needs_fill(bucket_id, child_path.as_inner());
        }

        Ok(())
    }
    fn delete_empty(&mut self, path: Path) -> Result<(), ModifyErr> {
        let Some((final_index, parent_path)) = path.split_last() else {
            return Err(ModifyErr::DeleteRoot);
        };

        let dest = self.trees.item.find_child_mut(parent_path);
        let dest = match dest {
            Ok(ChildFound::RootChildren(child_vec)) => child_vec,
            Ok(ChildFound::Joint(joint)) => &mut joint.next,
            Ok(ChildFound::Bucket(_)) | Err(UnknownPathSlice(_)) => {
                return Err(UnknownPath(path).into());
            }
        };

        let Some(target_elem_items) = dest.children().get(final_index) else {
            return Err(UnknownPath(path).into());
        };

        match target_elem_items {
            Child::Bucket(bucket) if !bucket.items.is_empty() => {
                return Err(ModifyErr::DeleteNonemptyBucket(path));
            }
            Child::Joint(joint) if !joint.next.is_empty() => {
                return Err(ModifyErr::DeleteNonemptyJoint(path));
            }
            Child::Bucket(_) | Child::Joint(_) => {}
        }

        // remove order first, in case it errors
        self.trees.order.remove(&path).map_err(|err| {
            err.unwrap_or_else(|| {
                unreachable!(
                    "DeleteRoot error from order should be detected when checking item nodes"
                )
            })
        })?;

        let bucket_id = match target_elem_items {
            Child::Bucket(bucket) => Some(bucket.id),
            Child::Joint(_) => None,
        };

        dest.remove(final_index);

        // update the cache for the removed node path
        self.bucket_paths.update_for_removed_path(&path, bucket_id);

        Ok(())
    }
    fn set_bucket_items(
        &mut self,
        new_contents: Vec<T>,
        bucket_path: &PathSlice,
    ) -> Result<(), ModifyErr> {
        let dest_bucket = match self.trees.item.find_bucket_mut(bucket_path) {
            Ok(Some(bucket)) => bucket,
            Ok(None) => Err(ModifyErr::FillJoint)?,
            Err(unknown) => Err(unknown.to_owned())?,
        };

        dest_bucket.items = new_contents;
        self.bucket_paths.remove_needs_fill(dest_bucket.id);

        Ok(())
    }
    fn set_filters(&mut self, new_filters: Vec<U>, path: Path) -> Result<(), ModifyErr> {
        let dest = self.trees.item.find_child_mut(&path);
        let (dest_filters, needs_fill_info) = match dest {
            Ok(ChildFound::RootChildren(_)) => Err(ModifyErr::FilterRoot)?,
            Ok(ChildFound::Joint(joint)) => (&mut joint.filters, Ok(&joint.next)),
            Ok(ChildFound::Bucket(bucket)) => (&mut bucket.filters, Err(bucket.id)),
            Err(UnknownPathSlice(_)) => return Err(UnknownPath(path).into()),
        };

        *dest_filters = new_filters;

        match needs_fill_info {
            Ok(joint_children) => {
                // target is joint, search for all child buckets
                Trees::visit_depth_first_items_at(path, joint_children, |elem| {
                    match elem.node_item {
                        Child::Bucket(bucket) => {
                            self.bucket_paths.add_needs_fill(bucket.id, elem.node_path);
                        }
                        Child::Joint(_) => {}
                    }
                });
            }
            Err(bucket_id) => {
                // target is bucket
                self.bucket_paths.add_needs_fill(bucket_id, &path);
            }
        }

        Ok(())
    }
    fn set_weight(&mut self, new_weight: u32, path: Path) -> Result<(), ModifyErr> {
        let Some((last_index, parent_path)) = path.split_last() else {
            return Err(ModifyErr::WeightRoot);
        };
        let dest = self.trees.item.find_child_mut(parent_path);
        let dest = match dest {
            Ok(ChildFound::RootChildren(child_vec)) => child_vec,
            Ok(ChildFound::Joint(joint)) => &mut joint.next,
            Ok(ChildFound::Bucket(_)) | Err(UnknownPathSlice(_)) => {
                return Err(UnknownPath(path).into());
            }
        };

        if last_index < dest.len() {
            dest.set_weight(last_index, new_weight);
            Ok(())
        } else {
            Err(UnknownPath(path).into())
        }
    }
    /// Returns `true` if all leaf buckets in the network have no items
    #[must_use]
    pub fn is_empty(&self) -> bool {
        let mut item_found = None;
        self.trees.visit_depth_first(|elem| {
            if item_found.is_some() {
                return;
            }
            match &elem.node_item {
                Child::Bucket(bucket) => {
                    if !bucket.items.is_empty() {
                        item_found = Some(());
                    }
                }
                Child::Joint(_) => {}
            }
        });
        item_found.is_none()
    }
}

mod bucket_paths_map {
    use crate::{
        BucketId,
        path::{Path, PathSlice},
    };
    use std::collections::{HashMap, HashSet};

    #[derive(Clone, Debug, Default)]
    pub(super) struct BucketPathsMap {
        /// Index into `cached_bucket_paths` for buckets needing fill
        ids_needing_fill: HashSet<BucketId>,
        /// Cache of `Paths` for buckets (may be empty at any time)
        cached_paths: HashMap<BucketId, Path>,
    }
    impl BucketPathsMap {
        pub(super) fn is_cache_missing_any_need_fill(&self) -> bool {
            self.ids_needing_fill
                .iter()
                .any(|id| !self.cached_paths.contains_key(id))
        }
        pub(super) fn iter_needs_fill(&self) -> impl Iterator<Item = BucketId> + '_ {
            self.ids_needing_fill.iter().copied()
        }
        pub(super) fn add_needs_fill(&mut self, id: BucketId, path: &PathSlice) {
            self.ids_needing_fill.insert(id);
            self.add_cached(id, path);
        }
        pub(super) fn remove_needs_fill(&mut self, id: BucketId) {
            self.ids_needing_fill.remove(&id);
        }
        pub(super) fn update_for_removed_path(
            &mut self,
            removed_path: &PathSlice,
            removed_bucket_id: Option<BucketId>,
        ) {
            let Self {
                ids_needing_fill,
                cached_paths,
            } = self;
            if let Some(id) = removed_bucket_id {
                ids_needing_fill.remove(&id);
                cached_paths.remove(&id);
            }
            for path in self.cached_paths.values_mut() {
                path.modify_for_removed(removed_path)
                    .expect("removed bucket path should already be removed from the path cache");
            }
        }
        pub(super) fn add_cached(&mut self, id: BucketId, path: &PathSlice) {
            match self.cached_paths.get(&id) {
                Some(existing) if &**existing == path => {}
                _ => {
                    self.cached_paths.insert(id, path.to_owned());
                }
            }
        }
        pub(super) fn get_cached(&self, id: BucketId) -> Option<&PathSlice> {
            self.cached_paths.get(&id).map(|v| &**v)
        }

        #[cfg(test)]
        pub(super) fn expose_cache_for_test(&self) -> impl Iterator<Item = (&BucketId, &Path)> {
            self.cached_paths.iter()
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
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

impl<T, U> Child<T, U> {
    fn get_filters(&self) -> &[U] {
        match self {
            Child::Bucket(bucket) => &bucket.filters,
            Child::Joint(joint) => &joint.filters,
        }
    }
}

/// Command to modify a network
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum ModifyCmd<T, U> {
    /// Add a new bucket
    AddBucket {
        /// Path for the new bucket
        new_path: Path,
    },
    /// Add a new bucket to the specified parent
    AddBucketTo {
        /// Parent path for the new bucket
        parent: Path,
    },
    /// Add a new joint
    AddJoint {
        /// Path for the new joint
        new_path: Path,
    },
    /// Add a new joint to the specified parent
    AddJointTo {
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
pub use modify_cmd_ref::ModifyCmdRef;
mod modify_cmd_ref {
    use crate::{ModifyCmd, order, path::PathSlice};

    /// Reference to a [`ModifyCmd`]
    ///
    /// See [`ModifyCmd`] for documentation on specific fields
    #[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
    #[expect(missing_docs, reason = "duplicate of `ModifyCmd`")]
    #[non_exhaustive]
    #[must_use]
    pub enum ModifyCmdRef<'a, T, U> {
        AddBucket {
            new_path: &'a PathSlice,
        },
        AddBucketTo {
            parent: &'a PathSlice,
        },
        AddJoint {
            new_path: &'a PathSlice,
        },
        AddJointTo {
            parent: &'a PathSlice,
        },
        DeleteEmpty {
            path: &'a PathSlice,
        },
        FillBucket {
            bucket: &'a PathSlice,
            new_contents: &'a [T],
        },
        SetFilters {
            path: &'a PathSlice,
            new_filters: &'a [U],
        },
        SetWeight {
            path: &'a PathSlice,
            new_weight: u32,
        },
        SetOrderType {
            path: &'a PathSlice,
            new_order_type: order::OrderType,
        },
    }
    impl<T, U> ModifyCmd<T, U> {
        /// Takes a reference to the inner [`Path`](`super::Path`) (if any)
        pub fn as_ref(&self) -> ModifyCmdRef<'_, T, U> {
            self.into()
        }
    }
    impl<'a, T, U> From<&'a ModifyCmd<T, U>> for ModifyCmdRef<'a, T, U> {
        fn from(value: &'a ModifyCmd<T, U>) -> Self {
            match value {
                ModifyCmd::AddBucket { new_path } => Self::AddBucket { new_path },
                ModifyCmd::AddBucketTo { parent } => Self::AddBucketTo { parent },
                ModifyCmd::AddJoint { new_path } => Self::AddJoint { new_path },
                ModifyCmd::AddJointTo { parent } => Self::AddJointTo { parent },
                ModifyCmd::DeleteEmpty { path } => Self::DeleteEmpty { path },
                ModifyCmd::FillBucket {
                    bucket,
                    new_contents,
                } => Self::FillBucket {
                    bucket,
                    new_contents,
                },
                ModifyCmd::SetFilters { path, new_filters } => {
                    Self::SetFilters { path, new_filters }
                }
                ModifyCmd::SetWeight { path, new_weight } => Self::SetWeight {
                    path,
                    new_weight: *new_weight,
                },
                ModifyCmd::SetOrderType {
                    path,
                    new_order_type,
                } => Self::SetOrderType {
                    path,
                    new_order_type: *new_order_type,
                },
            }
        }
    }
    impl<T, U> ModifyCmdRef<'_, T, U>
    where
        T: Clone,
        U: Clone,
    {
        /// Converts the inner [`PathSlice`] to an owned path (if any)
        #[must_use]
        pub fn to_owned(self) -> ModifyCmd<T, U> {
            match self {
                Self::AddBucket { new_path } => ModifyCmd::AddBucket {
                    new_path: new_path.to_owned(),
                },
                Self::AddBucketTo { parent } => ModifyCmd::AddBucketTo {
                    parent: parent.to_owned(),
                },
                Self::AddJoint { new_path } => ModifyCmd::AddJoint {
                    new_path: new_path.to_owned(),
                },
                Self::AddJointTo { parent } => ModifyCmd::AddJointTo {
                    parent: parent.to_owned(),
                },
                Self::DeleteEmpty { path } => ModifyCmd::DeleteEmpty {
                    path: path.to_owned(),
                },
                Self::FillBucket {
                    bucket,
                    new_contents,
                } => ModifyCmd::FillBucket {
                    bucket: bucket.to_owned(),
                    new_contents: new_contents.to_vec(),
                },
                Self::SetFilters { path, new_filters } => ModifyCmd::SetFilters {
                    path: path.to_owned(),
                    new_filters: new_filters.to_vec(),
                },
                Self::SetWeight { path, new_weight } => ModifyCmd::SetWeight {
                    path: path.to_owned(),
                    new_weight,
                },
                Self::SetOrderType {
                    path,
                    new_order_type,
                } => ModifyCmd::SetOrderType {
                    path: path.to_owned(),
                    new_order_type,
                },
            }
        }
    }
}

/// Error modifying the [`Network`]
#[derive(thiserror::Error)]
#[error(transparent)]
pub struct ModifyError(#[from] ModifyErr);
#[derive(Debug, thiserror::Error)]
enum ModifyErr {
    #[error(transparent)]
    UnknownPath(#[from] UnknownPath),
    #[error(transparent)]
    UnknownOrderPath(#[from] order::UnknownOrderPath),
    #[error(transparent)]
    UnknownBucketId(#[from] UnknownBucketId),
    #[error(transparent)]
    AddToBucket(#[from] CannotAddToBucket),

    #[error("cannot replace existing node: {0}")]
    AddAtPath(Path),
    #[error("cannot delete non-empty bucket: {0}")]
    DeleteNonemptyBucket(Path),
    #[error("cannot delete non-empty joint: {0}")]
    DeleteNonemptyJoint(Path),
    #[error("cannot delete the spigot (root node)")]
    DeleteRoot,
    #[error("cannot filter the spigot (root node)")]
    FilterRoot,
    #[error("cannot fill joint (only buckets have items)")]
    FillJoint,
    #[error("cannot weight the spigot (root node)")]
    WeightRoot,
}
impl std::fmt::Debug for ModifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ModifyError({self})")
    }
}

/// The specified path does not match a node (any of the joints, buckets, or root spigot)
#[derive(Debug, thiserror::Error)]
pub struct UnknownPath(Path);
impl std::fmt::Display for UnknownPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        UnknownPathSlice::new(&self.0).fmt(f)
    }
}

/// The specified path does not match a node (any of the joints, buckets, or root spigot)
#[derive(Debug, thiserror::Error, ref_cast::RefCastCustom)]
#[repr(transparent)]
#[error("unknown path: {0}")]
pub struct UnknownPathSlice(PathSlice);
impl UnknownPathSlice {
    #[ref_cast::ref_cast_custom]
    fn new(path: &PathSlice) -> &Self;

    /// Clones to create an owned version of the error
    fn to_owned(&self) -> UnknownPath {
        UnknownPath(self.0.to_owned())
    }
}

/// The specified bucket id does not match any bucket
#[derive(Clone, Copy, Debug, thiserror::Error)]
#[error("unknown bucket id: {}", .0.0)]
pub struct UnknownBucketId(BucketId);

/// Buckets cannot have filters or child joints or buckets
#[derive(Debug, thiserror::Error)]
#[error("cannot add to bucket: {0}")]
pub(crate) struct CannotAddToBucket(Path);

#[cfg(test)]
#[allow(clippy::panic)] // TODO use actual error handling for tests, `eyre` prints good details!
#[allow(clippy::unwrap_used)] // TODO
mod tests {
    pub(crate) use arb_rng::{PanicRng, decode_hex, fake_rng};
    pub(crate) use sync::run_with_timeout;

    // utils
    mod arb_network;
    mod arb_rng;
    mod script;
    mod sync;

    // test cases
    mod clap;
    mod modify_network;
    mod path;
    mod peek_effort;
    mod peek_pop_network;
    mod ser;
    mod view_table;
}
