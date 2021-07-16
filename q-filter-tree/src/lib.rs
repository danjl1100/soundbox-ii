//! [`Tree`] structure, where each node has a queue of items and a filter.

// only while building
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
mod order;

mod iter;
mod serde;

/// Numeric type for weighting nodes in the [`Tree`], used by to fuel [`OrderType`] algorithms
pub type Weight = u32;

/// Tree data structure, consisting of [`Node`]s with queues of items `T`, filter `F`
pub struct Tree<T, F> {
    root: Node<T, F>,
    next_sequence: id::Sequence,
}
impl<T, F> Tree<T, F> {
    /// Creates a tree with a single root node
    #[must_use]
    pub fn new() -> Self {
        let root_sequence = id::ROOT.sequence();
        let root = Node::new(root_sequence);
        Tree {
            root,
            next_sequence: root_sequence + 1,
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
        let node_path = node_path.into();
        let sequence = {
            let sequence = self.next_sequence;
            self.next_sequence += 1;
            sequence
        };
        let parent = self.get_node_mut(node_path)?;
        Ok(parent.add_child(node_path, weight, sequence))
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
