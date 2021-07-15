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

pub use error::{InvalidNodePath, PopError, RemoveError};
mod error;

use id::NodePathElem;
pub use id::{NodeId, NodePath};
mod id;

use node::Node;
mod node;

pub use order::Type as OrderType;
mod order;

/// Numeric type for weighting nodes in the [`Tree`], used by to fuel [`OrderType`] algorithms
pub type Weight = u32;

/// Tree data structure, consisting of nodes with queues of items `T`, filter `F`
pub struct Tree<T, F>
where
    F: Default,
{
    root: Node<T, F>,
    next_sequence: id::Sequence,
}
impl<T, F> Tree<T, F>
where
    F: Default,
{
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
    /// Adds an empty child node to the specified node, with optional weight
    ///
    /// # Errors
    /// Returns an error if the specified `NodeId` does not point to a valid node
    ///
    pub fn add_child(
        &mut self,
        node_id: &NodeId,
        weight: Option<Weight>,
    ) -> Result<NodeId, InvalidNodePath> {
        let sequence = {
            let sequence = self.next_sequence;
            self.next_sequence += 1;
            sequence
        };
        let parent = self.get_node_mut(node_id)?;
        Ok(parent.add_child(node_id, weight, sequence))
    }
    /// Sets the weight of the specified node
    ///
    /// # Errors
    /// Returns an error if the specified `NodeId` does not point to a valid node
    ///
    pub fn set_weight(&mut self, node_id: &NodeId, weight: Weight) -> Result<(), InvalidNodePath> {
        self.root.set_weight(node_id.into(), weight)
    }
    /// Returns the filter of the specified node
    ///
    /// # Errors
    /// Returns an error if the specified `NodeId` does not point to a valid node
    ///
    pub fn get_filter(&self, node_id: &NodeId) -> Result<&F, InvalidNodePath> {
        let node = self.get_node(node_id)?;
        Ok(&node.filter)
    }
    /// Sets the filter of the specified node
    ///
    /// # Errors
    /// Returns an error if the specified `NodeId` does not point to a valid node
    ///
    pub fn set_filter(&mut self, node_id: &NodeId, filter: F) -> Result<(), InvalidNodePath> {
        let node = self.get_node_mut(node_id)?;
        node.filter = filter;
        Ok(())
    }
    /// Sets the [`OrderType`] of the specified node
    ///
    /// # Errors
    /// Returns an error if the specified `NodeId` does not point to a valid node
    ///
    pub fn set_order(&mut self, node_id: &NodeId, order: OrderType) -> Result<(), InvalidNodePath> {
        let node = self.get_node_mut(node_id)?;
        node.set_order(order);
        Ok(())
    }
    /// Appends an item to the queue of the specified node
    ///
    /// # Errors
    /// Returns an error if the specified `NodeId` does not point to a valid node
    ///
    pub fn push_item(&mut self, node_id: &NodeId, item: T) -> Result<(), InvalidNodePath> {
        let node = self.get_node_mut(node_id)?;
        node.queue.push_back(item);
        Ok(())
    }
    /// Pops an item to the queue of the specified node
    ///
    /// # Errors
    /// Returns an error if the specified `NodeId` does not point to a valid node
    ///
    pub fn pop_item_from(
        &mut self,
        node_id: &NodeId,
    ) -> Result<Result<T, PopError<NodeId>>, InvalidNodePath> {
        let node = self.get_node_mut(node_id)?;
        Ok(node
            .pop_item()
            .map_err(|e| e.map_inner(|_| node_id.clone())))
    }
    /// Removes an empty node
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
}
impl<T, F> Default for Tree<T, F>
where
    F: Default,
{
    fn default() -> Self {
        Self::new()
    }
}
