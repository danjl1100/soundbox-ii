// soundbox-ii/q-filter-tree music playback sequencer *don't keep your sounds boxed up*
// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.
//
//! [`Tree`] structure, where each node has a queue of items and a filter.

// TODO: only while building
#![allow(dead_code)]
// teach me
#![deny(clippy::pedantic)]
// no unsafe
#![forbid(unsafe_code)]
// no unwrap
#![deny(clippy::unwrap_used)]
// no panic
#![deny(clippy::panic)]
// docs!
#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

use std::borrow::Cow;

use error::InvalidNodePath;
pub mod error;

use id::{ty, NodeId, NodePath};
pub mod id;

pub use node::meta::NodeInfoIntrinsic as NodeInfo;
pub use node::Node;
mod node;

mod weight_vec;

use node::RemoveResult;
pub use order::Type as OrderType;
pub mod order;

mod iter;
pub mod iter_mut;
/// Serialize/Deserialize functionality
pub mod serde {
    /// Error from deserializing a [`NodePathTyped`](`super::id::NodePathTyped`)
    pub type NodePathParseError = node_path::ParseError;
    /// Error from deserializing a [`NodeIdTyped`](`super::id::NodeIdTyped`)
    pub type NodeIdParseError = node_id::ParseError;
    pub use node_descriptor::NodeDescriptor;
    mod node_descriptor;
    mod node_id;
    mod node_path;
    mod tree;
}

pub mod refs;

/// Numeric type for weighting nodes in the [`Tree`], used by to fuel [`OrderType`] algorithms
pub type Weight = u32;

#[test]
fn tree_add_to_doc_tests() {
    let mut tree: Tree<_, _> = Tree::new();
    let root = tree.root_id();
    //
    let mut root_ref = root.try_ref(&mut tree);
    root_ref.filter = Some("filter value".to_string());
    let mut root_ref = root_ref.child_nodes().expect("root is chain");
    let child_blocked = root_ref.add_child(0);
    let child = root_ref.add_child_default();
    // initial weight `None` (0)
    child_blocked
        .try_ref(&mut tree)
        .expect("root exists")
        .push_item("apple");
    // initial weight `1`
    child
        .try_ref(&mut tree)
        .expect("child exists")
        .push_item("banana");
    //
    let mut root_ref = root.try_ref(&mut tree);
    assert_eq!(root_ref.pop_item(), Some(Cow::Owned("banana")));
    assert_eq!(root_ref.pop_item(), None);
    // unblock "child_blocked"
    child_blocked
        .try_ref(&mut tree)
        .expect("child_blocked exists")
        .set_weight(2);
    let child_unblocked = child_blocked;
    child_unblocked
        .try_ref(&mut tree)
        .expect("child_unblocked exists")
        .push_item("cashews");
    let mut root_ref = root.try_ref(&mut tree);
    assert_eq!(root_ref.pop_item(), Some(Cow::Owned("apple")));
    assert_eq!(root_ref.pop_item(), Some(Cow::Owned("cashews")));
    assert_eq!(root_ref.pop_item(), None);
}
/// Tree data structure, consisting of nodes with queues of items `T`, filter `F`
///
/// # Example
/// ```
/// use std::borrow::Cow;
/// use q_filter_tree::{Tree, error::PopError};
/// let mut tree: Tree<_, _> = Tree::new();
/// let root = tree.root_id();
/// //
/// let mut root_ref = root.try_ref(&mut tree);
/// root_ref.filter = Some("filter value".to_string());
/// let mut root_ref = root_ref.child_nodes().expect("root is chain");
/// let child_blocked = root_ref.add_child(0);
/// let child = root_ref.add_child_default();
/// // initial weight `None` (0)
/// child_blocked
///     .try_ref(&mut tree)
///     .expect("root exists")
///     .push_item("apple");
/// // initial weight `1`
/// child
///     .try_ref(&mut tree)
///     .expect("child exists")
///     .push_item("banana");
/// //
/// let mut root_ref = root.try_ref(&mut tree);
/// assert_eq!(root_ref.pop_item(), Some(Cow::Owned("banana")));
/// assert_eq!(root_ref.pop_item(), None);
/// // unblock "child_blocked"
/// child_blocked
///     .try_ref(&mut tree)
///     .expect("child_blocked exists")
///     .set_weight(2);
/// let child_unblocked = child_blocked;
/// child_unblocked
///     .try_ref(&mut tree)
///     .expect("child_unblocked exists")
///     .push_item("cashews");
/// let mut root_ref = root.try_ref(&mut tree);
/// assert_eq!(root_ref.pop_item(), Some(Cow::Borrowed(&"apple")));
/// assert_eq!(root_ref.pop_item(), Some(Cow::Borrowed(&"cashews")));
/// assert_eq!(root_ref.pop_item(), None);
/// ```
///
#[derive(Debug)]
pub struct Tree<T, F> {
    root: Node<T, F>,
    sequence_counter: node::SequenceCounter,
}
impl<T, F> Tree<T, F>
where
    F: Default,
{
    /// Creates a tree with a single root node
    #[must_use]
    pub fn new() -> Self {
        Self::new_with_root(node::meta::NodeInfoIntrinsic::default())
    }
}
impl<T, F> Tree<T, F> {
    /// Creates a tree with the specified root info
    pub(crate) fn new_with_root(node_info: node::meta::NodeInfoIntrinsic<T, F>) -> Self {
        let (root, sequence_counter) = node_info.construct_root();
        Tree {
            root,
            sequence_counter,
        }
    }
    /// Returns the [`NodeId`] of the root node
    pub fn root_id(&self) -> NodeId<ty::Root> {
        #![allow(clippy::unused_self)]
        id::ROOT
    }
    /// Removes an empty node
    ///
    /// **Note:** Explicit [`NodeId`] is required to preserve idempotency.
    /// E.g. Removing a node may change the path of adjacent nodes.
    ///
    /// # Errors
    /// Returns an error if the specified `NodeId` does not point to a valid node,
    ///  or if the node has existing children.
    ///
    pub fn remove_node(
        &mut self,
        node_id: &NodeId<ty::Child>,
    ) -> Result<RemoveResult<T, F, NodeId<ty::Child>>, InvalidNodePath> {
        let err_child_path_invalid = || InvalidNodePath::from(node_id.clone().into_inner());
        // calculate parent path
        let node_id_cloned = NodePath::from(node_id.clone());
        let (parent_id, last_elem) = node_id_cloned.into_parent();
        // remove child from parent
        let mut parent = parent_id.try_ref(self)?;
        match &mut parent.children {
            node::Children::Chain(chain) => chain
                .remove_child(last_elem, node_id)
                .map(|remove_result| remove_result.map_err(|e| e.map_id(|_| node_id.clone())))
                .map_err(|_| err_child_path_invalid()),
            node::Children::Items(_) => Err(err_child_path_invalid()),
        }
    }
    /// Calculate the total node count (including the root)
    pub fn sum_node_count(&self) -> usize {
        self.root.children.sum_node_count()
    }
}
impl<T: Clone, F> Tree<T, F> {
    /// Pops items from node queues, or if no queue is available, returns references from item-nodes
    pub fn pop_item(&mut self) -> Option<Cow<'_, T>> {
        self.root.pop_item()
    }
}
impl<T, F> Default for Tree<T, F>
where
    F: Default,
{
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use crate::{OrderType, Tree};
    use std::borrow::Cow;
    #[test]
    fn simplest_items() {
        let mut tree: Tree<_, ()> = Tree::new();
        let root = tree.root_id();
        let mut root_ref = root.try_ref(&mut tree);
        assert_eq!(root_ref.get_order_type(), OrderType::InOrder);
        root_ref.set_child_items_uniform(vec!["hey", "this", "is"]);
        for _ in 0..200 {
            assert_eq!(tree.pop_item(), Some(Cow::Owned("hey")));
            assert_eq!(tree.pop_item(), Some(Cow::Owned("this")));
            assert_eq!(tree.pop_item(), Some(Cow::Owned("is")));
        }
        let mut root_ref = root.try_ref(&mut tree);
        root_ref.push_item("special");
        root_ref.push_item("item");
        assert_eq!(tree.pop_item(), Some(Cow::Owned("special")));
        assert_eq!(tree.pop_item(), Some(Cow::Owned("item")));
        for _ in 0..200 {
            assert_eq!(tree.pop_item(), Some(Cow::Owned("hey")));
            assert_eq!(tree.pop_item(), Some(Cow::Owned("this")));
            assert_eq!(tree.pop_item(), Some(Cow::Owned("is")));
        }
    }
    #[test]
    #[allow(clippy::similar_names)]
    fn chain() {
        let mut tree: Tree<_, ()> = Tree::new();
        let root = tree.root_id();
        // root > child_a child_b
        let mut root_ref = root.try_ref(&mut tree);
        let mut root_ref_child_nodes = root_ref.child_nodes().expect("root is chain");
        let child_a = root_ref_child_nodes.add_child_default();
        let child_b = root_ref_child_nodes.add_child_default();
        let child_c = root_ref_child_nodes.add_child_default();
        // root > child_a > child_a_a
        let mut child_a_ref = child_a.try_ref(&mut tree).expect("child_a exists");
        let child_a_a = child_a_ref
            .child_nodes()
            .expect("child_a is chain")
            .add_child_default();
        // root > child_a > child_a_a [ items ]
        let mut child_a_a_ref = child_a_a.try_ref(&mut tree).expect("child_a_a exists");
        child_a_a_ref.set_child_items_uniform(vec!["aa1", "aa2"]);
        // root > child_b > child_b_b, child_b_z
        let mut child_b_ref = child_b.try_ref(&mut tree).expect("child_b exists");
        let mut child_b_ref_child_nodes = child_b_ref.child_nodes().expect("=child_b is chain");
        let child_b_b = child_b_ref_child_nodes.add_child_default();
        let child_b_z = child_b_ref_child_nodes.add_child_default();
        // root > child_b > child_b_b [ items ]
        let mut child_b_b_ref = child_b_b.try_ref(&mut tree).expect("child_b_b exists");
        child_b_b_ref.set_child_items_uniform(vec!["bb1", "bb2"]);
        // root > child_b > child_b_z [ items ]
        let mut child_b_z_ref = child_b_z.try_ref(&mut tree).expect("child_b_z exists");
        child_b_z_ref.set_child_items_uniform(vec!["bz1", "bz2"]);
        // root > child_c [ items ]
        let mut child_c_ref = child_c.try_ref(&mut tree).expect("child_c exists");
        child_c_ref.set_child_items_uniform(vec!["cc1", "cc2"]);
        //
        for _ in 0..100 {
            assert_eq!(tree.pop_item(), Some(Cow::Owned("aa1")));
            assert_eq!(tree.pop_item(), Some(Cow::Owned("bb1")));
            assert_eq!(tree.pop_item(), Some(Cow::Owned("cc1")));
            //
            assert_eq!(tree.pop_item(), Some(Cow::Owned("aa2")));
            assert_eq!(tree.pop_item(), Some(Cow::Owned("bz1")));
            assert_eq!(tree.pop_item(), Some(Cow::Owned("cc2")));
            //
            assert_eq!(tree.pop_item(), Some(Cow::Owned("aa1")));
            assert_eq!(tree.pop_item(), Some(Cow::Owned("bb2")));
            assert_eq!(tree.pop_item(), Some(Cow::Owned("cc1")));
            //
            assert_eq!(tree.pop_item(), Some(Cow::Owned("aa2")));
            assert_eq!(tree.pop_item(), Some(Cow::Owned("bz2")));
            assert_eq!(tree.pop_item(), Some(Cow::Owned("cc2")));
        }
    }
}
