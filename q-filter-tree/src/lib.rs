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

use error::{InvalidNodePath, PopError, RemoveError};
pub mod error;

use id::{NodeId, NodeIdBuilder, NodePath, NodePathElem};
pub mod id;

pub use node::Node;
mod node;

pub use order::Type as OrderType;
pub mod order;

mod iter;
mod serde {
    mod node_path;
    mod tree;
}

/// Numeric type for weighting nodes in the [`Tree`], used by to fuel [`OrderType`] algorithms
pub type Weight = u32;

#[test]
fn tree_add_to_doc_tests() {
    let mut tree: Tree<_, _> = Tree::new();
    let root = tree.root_id();
    //
    assert!(tree.get_child_mut(&root).is_err());
    let mut root_ref = tree.get_mut(&root).expect("root exists");
    *root_ref.filter() = Some("filter value".to_string());
    let child_blocked = root_ref.add_child(None);
    let child = root_ref.add_child(Some(1));
    // initial weight `None` (0)
    tree.get_mut(&child_blocked)
        .expect("root exists")
        .push_item("apple");
    // initial weight `1`
    tree.get_mut(&child)
        .expect("child exists")
        .push_item("banana");
    //
    let mut root_ref = tree.get_mut(&root).expect("root exists");
    assert_eq!(root_ref.pop_item(), Ok("banana"));
    assert_eq!(root_ref.pop_item(), Err(PopError::Empty((*root).clone())));
    // unblock "child_blocked"
    tree.get_child_mut(&child_blocked)
        .expect("child_blocked exists")
        .set_weight(2);
    let child_unblocked = child_blocked;
    tree.get_child_mut(&child_unblocked)
        .expect("child_unblocked exists")
        .push_item("cashews");
    let mut root_ref = tree.get_mut(&root).expect("root exists");
    assert_eq!(root_ref.pop_item(), Ok("apple"));
    assert_eq!(root_ref.pop_item(), Ok("cashews"));
}
/// Tree data structure, consisting of [`Node`]s with queues of items `T`, filter `F`
///
/// # Example
/// ```
/// use q_filter_tree::{Tree, error::PopError};
/// let mut tree: Tree<_,()> = Tree::new();
/// let root = tree.root_id();
/// //
/// let child_blocked = tree.add_child(&root, None).expect("root exists");
/// let child = tree.add_child(&root, Some(1)).expect("root exists");
/// // initial weight `None` (0)
/// tree.push_item(&child_blocked, "apple").expect("childBlocked exists");
/// // initial weight `1`
/// tree.push_item(&child, "banana").expect("child exists");
/// //
/// assert_eq!(
///     tree.pop_item_from(&root).expect("root exists"),
///     Ok("banana")
/// );
/// assert_eq!(
///     tree.pop_item_from(&root).expect("root exists"),
///     Err(PopError::Empty((*root).clone()))
/// );
/// // unblock "child_blocked"
/// tree.set_weight(&child_blocked, 2);
/// let child_unblocked = child_blocked;
/// assert_eq!(
///     tree.pop_item_from(&root).expect("root exists"),
///     Ok("apple"),
/// );
/// ```
///
pub struct Tree<T, F> {
    root: Node<T, F>,
    sequence_counter: node::SequenceCounter,
}
impl<T, F> Tree<T, F> {
    /// Creates a tree with a single root node
    #[must_use]
    pub fn new() -> Self {
        let (root, sequence_counter) = Node::new_root();
        Tree {
            root,
            sequence_counter,
        }
    }
    /// Returns the [`NodeId`] of the root node
    pub fn root_id(&self) -> NodeId {
        #![allow(clippy::unused_self)]
        id::ROOT
    }
    fn get_node<'a, P>(&self, node_path: &'a P) -> Result<&Node<T, F>, InvalidNodePath>
    where
        &'a P: Into<&'a [NodePathElem]>,
    {
        self.root.get_child(node_path.into())
    }
    fn get_node_mut<'a, P>(&mut self, node_path: &'a P) -> Result<&mut Node<T, F>, InvalidNodePath>
    where
        &'a P: Into<&'a [NodePathElem]>,
    {
        self.root.get_child_mut(node_path.into())
    }
    fn get_node_and_next_id<'a, P>(
        &self,
        node_path: &'a P,
    ) -> Result<(&Node<T, F>, Option<NodeId>), InvalidNodePath>
    where
        &'a P: Into<&'a [NodePathElem]>,
    {
        self.root
            .get_child_and_next_id(node_path.into())
            .map(|(node, builder)| {
                let next_id = builder.map(NodeIdBuilder::finish);
                (node, next_id)
            })
    }
    /// Returns `NodeRef` to the specified `NodeId`
    ///
    /// # Errors
    /// Returns an error if the specified `NodeId` does not point to a valid node
    ///
    pub fn get_mut<'a, P>(
        &mut self,
        path: &'a P,
    ) -> Result<NodeRefMut<'_, 'a, T, F>, InvalidNodePath>
    where
        &'a P: Into<&'a NodePath>,
    {
        let path = path.into();
        let node = self.root.get_child_mut(path.into())?;
        let sequence_counter = &mut self.sequence_counter;
        Ok(NodeRefMut {
            node,
            path,
            sequence_counter,
        })
    }
    /// Returns `NodeRef` to the specified `NodeId`
    ///
    /// # Errors
    /// Returns an error if the specified `NodeId` does not point to a valid **child** node
    ///
    pub fn get_child_mut<'a, P>(
        &mut self,
        path: &'a P,
    ) -> Result<NodeRefMutWeighted<'_, 'a, T, F>, InvalidNodePath>
    where
        &'a P: Into<&'a NodePath>,
    {
        let path = path.into();
        let (node, weight) = self.root.get_child_and_weight_mut(path.into())?;
        let weight = weight.ok_or(path)?;
        let sequence_counter = &mut self.sequence_counter;
        Ok(NodeRefMutWeighted(
            NodeRefMut {
                node,
                path,
                sequence_counter,
            },
            weight,
        ))
    }
    /// Adds an empty child node to the specified node, with optional weight
    ///
    /// # Errors
    /// Returns an error if the specified `NodeId` does not point to a valid node
    ///
    pub fn add_child<'a, P>(
        &mut self,
        node_path: &'a P,
        weight: Option<Weight>,
    ) -> Result<NodeId, InvalidNodePath>
    where
        &'a P: Into<&'a NodePath>,
    {
        Ok(self.get_mut(node_path)?.add_child(weight))
    }
    /// Sets the weight of the specified node
    ///
    /// # Errors
    /// Returns an error if the specified `NodeId` does not point to a valid node
    ///
    pub fn set_weight<'a, P>(
        &mut self,
        node_path: &'a P,
        weight: Weight,
    ) -> Result<(), InvalidNodePath>
    where
        &'a P: Into<&'a [NodePathElem]>,
    {
        self.root.set_weight(node_path.into(), weight)
    }
    /// Returns the filter of the specified node
    ///
    /// # Errors
    /// Returns an error if the specified `NodeId` does not point to a valid node
    ///
    pub fn get_filter<'a, P>(&self, node_path: &'a P) -> Result<Option<&F>, InvalidNodePath>
    where
        &'a P: Into<&'a [NodePathElem]>,
    {
        let node = self.get_node(node_path)?;
        Ok(node.filter.as_ref())
    }
    /// Sets the filter of the specified node
    ///
    /// # Errors
    /// Returns an error if the specified `NodeId` does not point to a valid node
    ///
    pub fn set_filter<'a, P>(&mut self, node_path: &'a P, filter: F) -> Result<(), InvalidNodePath>
    where
        &'a P: Into<&'a [NodePathElem]>,
    {
        let node = self.get_node_mut(node_path)?;
        node.filter.replace(filter);
        Ok(())
    }
    /// Removes the filter of the specified node
    ///
    /// # Errors
    /// Returns an error if the specified `NodeId` does not point to a valid node
    ///
    pub fn clear_filter<'a, P>(&mut self, node_path: &'a P) -> Result<(), InvalidNodePath>
    where
        &'a P: Into<&'a [NodePathElem]>,
    {
        let node = self.get_node_mut(node_path)?;
        node.filter.take();
        Ok(())
    }
    /// Sets the [`OrderType`] of the specified node
    ///
    /// # Errors
    /// Returns an error if the specified `NodeId` does not point to a valid node
    ///
    pub fn set_order<'a, P>(
        &mut self,
        node_path: &'a P,
        order: OrderType,
    ) -> Result<(), InvalidNodePath>
    where
        &'a P: Into<&'a [NodePathElem]>,
    {
        let node = self.get_node_mut(node_path)?;
        node.set_order(order);
        Ok(())
    }
    /// Appends an item to the queue of the specified node
    ///
    /// # Errors
    /// Returns an error if the specified `NodeId` does not point to a valid node
    ///
    pub fn push_item<'a, P>(&mut self, node_path: &'a P, item: T) -> Result<(), InvalidNodePath>
    where
        &'a P: Into<&'a [NodePathElem]>,
    {
        let node = self.get_node_mut(node_path)?;
        node.queue.push_back(item);
        Ok(())
    }
    /// Pops an item to the queue of the specified node
    ///
    /// # Errors
    /// Returns an error if the specified `NodeId` does not point to a valid node
    ///
    pub fn pop_item_from<'a, P>(
        &mut self,
        node_path: &'a P,
    ) -> Result<Result<T, PopError<NodePath>>, InvalidNodePath>
    where
        &'a P: Into<&'a NodePath>,
    {
        let node_path = node_path.into();
        let node = self.get_node_mut(node_path)?;
        Ok(node
            .pop_item()
            .map_err(|e| e.map_inner(|_| node_path.clone())))
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
    pub fn remove_node(&mut self, node_id: &NodeId) -> Result<(), RemoveError> {
        if let Some((parent_id, last_elem)) = node_id.parent() {
            let parent = self.get_node_mut(&parent_id)?;
            parent
                .remove_child(last_elem, node_id)
                .map(|_| ())
                .map_err(|e| e.attach_id(node_id))
        } else {
            Err(RemoveError::Root(node_id.clone().into()))
        }
    }
    /// Calculate the total node count (including the root)
    pub fn sum_node_count(&self) -> usize {
        self.root.sum_node_count()
    }
}
impl<T, F> Default for Tree<T, F> {
    fn default() -> Self {
        Self::new()
    }
}

/// Mutable reference to a [`Node`]
pub struct NodeRefMut<'a, 'b, T, F> {
    node: &'a mut Node<T, F>,
    path: &'b NodePath,
    sequence_counter: &'a mut node::SequenceCounter,
}
impl<'a, 'b, T, F> NodeRefMut<'a, 'b, T, F> {
    /// Adds an empty child node, with optional weight
    pub fn add_child(&mut self, weight: Option<Weight>) -> NodeId {
        let (child_part, sequence) = self.node.add_child(weight, &mut self.sequence_counter);
        self.path.extend(child_part).with_sequence(sequence)
    }
    // /// Mutable access to queue
    // pub fn queue(&mut self) -> &mut std::collections::VecDeque<T> {
    //     &mut self.node.queue
    // }
    /// Mutable access to filter
    pub fn filter(&mut self) -> &mut Option<F> {
        &mut self.node.filter
    }
    /// Appends an item to the queue
    pub fn push_item(&mut self, item: T) {
        self.node.queue.push_back(item);
    }
    /// Pops an item from the queue
    ///
    /// # Errors
    /// Returns an error if the pop failed
    ///
    pub fn pop_item(&mut self) -> Result<T, PopError<NodePath>> {
        self.node
            .pop_item()
            .map_err(|e| e.map_inner(|_| self.path.clone()))
    }
}

/// Mutable reference to a [`Node`] with an associated [`Weight`]
pub struct NodeRefMutWeighted<'a, 'b, T, F>(NodeRefMut<'a, 'b, T, F>, &'a mut Weight);
impl<'a, 'b, T, F> NodeRefMutWeighted<'a, 'b, T, F> {
    /// Sets the weight
    pub fn set_weight(&mut self, weight: Weight) {
        *self.1 = weight;
    }
}
impl<'a, 'b, T, F> std::ops::Deref for NodeRefMutWeighted<'a, 'b, T, F> {
    type Target = NodeRefMut<'a, 'b, T, F>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<'a, 'b, T, F> std::ops::DerefMut for NodeRefMutWeighted<'a, 'b, T, F> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
