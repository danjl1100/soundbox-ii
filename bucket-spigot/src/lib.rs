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

use path::Path;
use std::collections::HashSet;

pub mod clap;
pub mod path;

pub mod order;

/// Group of buckets with a central spigot
#[derive(Clone, Debug, Default)]
pub struct Network<T, U> {
    root: Vec<Child<T, U>>,
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
                let _path = self.add_child(Child::bucket(), parent)?;
                Ok(())
            }
            ModifyCmd::AddJoint { parent } => {
                let _path = self.add_child(Child::joint(), parent)?;
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

        // add order for child (fails if node/order structures are not identical)
        let child_index = self.root_order.add(parent_path.as_ref())?;

        let child_index_expected = current_children.len();
        assert_eq!(
            child_index, child_index_expected,
            "order nodes should match item nodes"
        );

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
            self.buckets_needing_fill.insert(child_path.clone());
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
        if bucket_path_iter.next().is_some() {
            return Err(UnknownPath(bucket_path).into());
        }

        self.buckets_needing_fill.remove(&bucket_path);

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

            let mut joint_path_buf = joint_path;
            Self::add_buckets_need_fill_under(
                &mut joint_path_buf,
                &mut self.buckets_needing_fill,
                &*current_children,
            );

            Ok(())
        } else {
            Err(ModifyErr::FilterRoot)?
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
                    Self::add_buckets_need_fill_under(path, buckets_needing_fill, &joint.children);
                }
            }
            assert_eq!(path.pop(), Some(index), "should contain the pushed index");
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
    UnknownOrderPath(order::UnknownOrderPath),
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
    pub(crate) use arb_rng::PanicRng;

    // utils
    mod arb_rng;
    mod script;

    // test cases
    mod modify_network;
    mod peek_effort;
    mod peek_pop_network;
}
