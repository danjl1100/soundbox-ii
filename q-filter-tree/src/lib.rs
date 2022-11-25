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
/// Result for removing a node (when node is indeed found)
pub type RemoveResult<T, F> = node::RemoveResult<T, F, RemoveError>;
/// Error removing a node (when node is indeed found)
pub type RemoveError = error::RemoveError<NodeId<ty::Child>>;
pub use error::RemoveError as RemoveErrorInner;
pub use node::RemoveResult as RemoveResultInner;
mod node;

pub use weight_vec::Weight;
pub mod weight_vec;

pub use order::Type as OrderType;
pub mod order;

pub mod iter;
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
    let item = |seq, item| SequenceAndItem::new(seq, Cow::Owned(item));
    //
    let mut root_ref = root.try_ref(&mut tree);
    assert_eq!(root_ref.pop_item(), Some(item(2, "banana")));
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
    assert_eq!(root_ref.pop_item(), Some(item(1, "apple")));
    assert_eq!(root_ref.pop_item(), Some(item(1, "cashews")));
    assert_eq!(root_ref.pop_item(), None);
}
/// Tree data structure, consisting of nodes with queues of items `T`, filter `F`
///
/// # Example
/// ```
/// use std::borrow::Cow;
/// use q_filter_tree::{Tree, SequenceAndItem};
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
/// let item = |seq, item| SequenceAndItem::new(seq, Cow::Owned(item));
/// //
/// let mut root_ref = root.try_ref(&mut tree);
/// assert_eq!(root_ref.pop_item(), Some(item(2, "banana")));
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
/// assert_eq!(root_ref.pop_item(), Some(item(1, &"apple")));
/// assert_eq!(root_ref.pop_item(), Some(item(1, &"cashews")));
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
    /// Creates a tree with the specified filter on the root node
    pub fn new_with_filter(root_filter: F) -> Self {
        let node_info = node::meta::NodeInfoIntrinsic::default_with_filter(root_filter);
        Self::new_with_root(node_info)
    }
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
    /// Returns the root [`Node`]
    pub fn root_node_shared(&self) -> &Node<T, F> {
        &self.root
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
    ) -> Result<RemoveResult<T, F>, InvalidNodePath> {
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
    pub fn pop_item(&mut self) -> Option<SequenceAndItem<Cow<'_, T>>> {
        self.root.pop_item()
    }
    /// Refreshes `prefill` on all nodes
    pub fn refresh_prefill(&mut self) {
        use iter::IterMut;
        use shared::{IgnoreNever, Never};
        self.enumerate_mut()
            .with_all(|_path, mut node| {
                node.update_queue_prefill();
                Ok::<_, Never>(())
            })
            .ignore_never();
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
/// Mutable reference to a [`Tree`], to ensure [`Tree::refresh_prefill`] is called on [`Drop`]
///
/// See [`Tree::guard`] for construction.
pub struct TreeGuard<'a, T: Clone, F>(&'a mut Tree<T, F>);
impl<T: Clone, F> Tree<T, F> {
    /// Mutable handle to the [`Tree`], usable in [`id::NodeIdTyped::try_ref`] via [`AsMut`]
    pub fn guard(&mut self) -> TreeGuard<'_, T, F> {
        TreeGuard(self)
    }
}
impl<'a, T: Clone, F> Drop for TreeGuard<'a, T, F> {
    fn drop(&mut self) {
        self.0.refresh_prefill();
    }
}
impl<T, F> AsMut<Tree<T, F>> for Tree<T, F> {
    fn as_mut(&mut self) -> &mut Tree<T, F> {
        self
    }
}
impl<'a, T: Clone, F> AsMut<Tree<T, F>> for TreeGuard<'a, T, F> {
    fn as_mut(&mut self) -> &mut Tree<T, F> {
        self.0
    }
}

/// Item and the source node's [`Sequence`](`id::Sequence`)
#[derive(Clone, Debug, ::serde::Serialize, ::serde::Deserialize, PartialEq, Eq)]
pub struct SequenceAndItem<T>(id::Sequence, T);
impl<T> SequenceAndItem<T> {
    /// Constructs an instance using the specified `sequence`
    pub fn new(sequence: id::Sequence, item: T) -> Self {
        Self(sequence, item)
    }
    /// Returns a closure for constructing `SequenceAndItem` using the specified `sequence`
    pub fn new_fn(sequence: id::Sequence) -> impl Fn(T) -> Self {
        move |item| Self(sequence, item)
    }
    /// Maps the inner item using the specified function
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> SequenceAndItem<U> {
        let Self(seq, item) = self;
        let item = f(item);
        SequenceAndItem(seq, item)
    }
    /// Returns the `node` sequence number originating this item
    pub fn sequence_num(&self) -> id::Sequence {
        self.0
    }
    /// Returns only the item
    pub fn into_item(self) -> T {
        self.1
    }
    /// Returns the constituent parts
    pub fn into_parts(self) -> (id::Sequence, T) {
        (self.0, self.1)
    }
}
impl<T, U> AsRef<U> for SequenceAndItem<T>
where
    T: AsRef<U>,
    U: ?Sized,
{
    fn as_ref(&self) -> &U {
        self.1.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use crate::{OrderType, SequenceAndItem, Tree};
    use std::borrow::Cow;
    #[test]
    fn simplest_items() {
        let mut tree: Tree<_, ()> = Tree::new();
        let root = tree.root_id();
        let mut root_ref = root.try_ref(&mut tree);
        assert_eq!(root_ref.get_order_type(), OrderType::InOrder);
        root_ref.overwrite_child_items_uniform(vec!["hey", "this", "is"]);
        let item = |s| SequenceAndItem::new(0, Cow::Owned(s));
        for _ in 0..200 {
            assert_eq!(tree.pop_item(), Some(item("hey")));
            assert_eq!(tree.pop_item(), Some(item("this")));
            assert_eq!(tree.pop_item(), Some(item("is")));
        }
        let mut root_ref = root.try_ref(&mut tree);
        root_ref.push_item("special");
        root_ref.push_item("item");
        assert_eq!(tree.pop_item(), Some(item("special")));
        assert_eq!(tree.pop_item(), Some(item("item")));
        for _ in 0..200 {
            assert_eq!(tree.pop_item(), Some(item("hey")));
            assert_eq!(tree.pop_item(), Some(item("this")));
            assert_eq!(tree.pop_item(), Some(item("is")));
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
        child_a_a_ref.overwrite_child_items_uniform(vec!["aa1", "aa2"]);
        // root > child_b > child_b_b, child_b_z
        let mut child_b_ref = child_b.try_ref(&mut tree).expect("child_b exists");
        let mut child_b_ref_child_nodes = child_b_ref.child_nodes().expect("child_b is chain");
        let child_b_b = child_b_ref_child_nodes.add_child_default();
        let child_b_z = child_b_ref_child_nodes.add_child_default();
        // root > child_b > child_b_b [ items ]
        let mut child_b_b_ref = child_b_b.try_ref(&mut tree).expect("child_b_b exists");
        child_b_b_ref.overwrite_child_items_uniform(vec!["bb1", "bb2"]);
        // root > child_b > child_b_z [ items ]
        let mut child_b_z_ref = child_b_z.try_ref(&mut tree).expect("child_b_z exists");
        child_b_z_ref.overwrite_child_items_uniform(vec!["bz1", "bz2"]);
        // root > child_c [ items ]
        let mut child_c_ref = child_c.try_ref(&mut tree).expect("child_c exists");
        child_c_ref.overwrite_child_items_uniform(vec!["cc1", "cc2"]);
        //
        let item = |seq, item| SequenceAndItem::new(seq, Cow::Owned(item));
        for _ in 0..100 {
            assert_eq!(tree.pop_item(), Some(item(4, "aa1")));
            assert_eq!(tree.pop_item(), Some(item(5, "bb1")));
            assert_eq!(tree.pop_item(), Some(item(3, "cc1")));
            //
            assert_eq!(tree.pop_item(), Some(item(4, "aa2")));
            assert_eq!(tree.pop_item(), Some(item(6, "bz1")));
            assert_eq!(tree.pop_item(), Some(item(3, "cc2")));
            //
            assert_eq!(tree.pop_item(), Some(item(4, "aa1")));
            assert_eq!(tree.pop_item(), Some(item(5, "bb2")));
            assert_eq!(tree.pop_item(), Some(item(3, "cc1")));
            //
            assert_eq!(tree.pop_item(), Some(item(4, "aa2")));
            assert_eq!(tree.pop_item(), Some(item(6, "bz2")));
            assert_eq!(tree.pop_item(), Some(item(3, "cc2")));
        }
    }
}
