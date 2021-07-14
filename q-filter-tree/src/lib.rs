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

use node::Node;
mod node;

use id::{NodeId, NodeIdElem};
mod id;

mod order;

/// Numeric type for weighting nodes in the [`Tree`], used by to fuel [`Merge`] algorithms
pub type Weight = u32;

/// Error for an invalid [`NodeId`]
#[derive(Debug)]
pub struct InvalidNodeId(NodeId);
impl From<&[NodeIdElem]> for InvalidNodeId {
    fn from(node_id: &[NodeIdElem]) -> Self {
        Self(node_id.to_vec().into())
    }
}

/// Tree data structure, consisting of nodes with queues of items `T`, filter `F`
pub struct Tree<T, F>
where
    F: Default,
{
    root: Node<T, F>,
}
impl<T, F> Tree<T, F>
where
    F: Default,
{
    /// Creates a tree with a single root node
    #[must_use]
    pub fn new() -> Self {
        let root = Node::default();
        Tree { root }
    }
    /// Returns the [`NodeId`] of the root node
    pub fn root_id(&self) -> NodeId {
        #![allow(clippy::unused_self)]
        id::ROOT
    }
    fn get_node(&self, node_id: &NodeId) -> Result<&Node<T, F>, InvalidNodeId> {
        self.root.get_child(node_id.into())
    }
    fn get_node_mut(&mut self, node_id: &NodeId) -> Result<&mut Node<T, F>, InvalidNodeId> {
        self.root.get_child_mut(node_id.into())
    }
    /// Adds an empty child node to the specified node, with optional weight
    ///
    /// # Errors
    /// Returns error if no node matches the `node_id`.
    ///
    pub fn add_child(
        &mut self,
        node_id: &NodeId,
        weight: Option<Weight>,
    ) -> Result<NodeId, InvalidNodeId> {
        let parent = self.get_node_mut(node_id)?;
        Ok(parent.add_child(node_id, weight))
    }
    /// Sets the weight of the specified node
    ///
    /// # Errors
    /// Returns error if no node matches the `node_id`.
    ///
    pub fn set_weight(&mut self, node_id: &NodeId, weight: Weight) -> Result<(), InvalidNodeId> {
        self.root.set_weight(node_id.into(), weight)
    }
    /// Returns the filter of the specified node
    ///
    /// # Errors
    /// Returns error if no node matches the `node_id`.
    ///
    pub fn get_filter(&self, node_id: &NodeId) -> Result<&F, InvalidNodeId> {
        let node = self.get_node(node_id)?;
        Ok(&node.filter)
    }
    /// Sets the filter of the specified node
    ///
    /// # Errors
    /// Returns error if no node matches the `node_id`.
    ///
    pub fn set_filter(&mut self, node_id: &NodeId, filter: F) -> Result<(), InvalidNodeId> {
        let node = self.get_node_mut(node_id)?;
        node.filter = filter;
        Ok(())
    }
    /// Appends an item to the queue of the specified node
    ///
    /// # Errors
    /// Returns error if no node matches the `node_id`.
    ///
    pub fn push_item(&mut self, node_id: &NodeId, item: T) -> Result<(), InvalidNodeId> {
        let node = self.get_node_mut(node_id)?;
        node.queue.push_back(item);
        Ok(())
    }
    /// Pops an item to the queue of the specified node
    ///
    /// # Errors
    /// Returns error if no node matches the `node_id`.
    ///
    pub fn pop_item_from(
        &mut self,
        node_id: &NodeId,
    ) -> Result<Result<T, PopError<NodeId>>, InvalidNodeId> {
        let node = self.get_node_mut(node_id)?;
        Ok(node
            .pop_item()
            .map_err(|e| e.map_inner(|_| node_id.clone())))
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

/// Error from the item-pop operation
#[derive(Debug, PartialEq, Eq)]
pub enum PopError<T> {
    /// Terminal node has an empty queue (needs push)
    Empty(T),
    /// Child nodes are not allowed (all weights = 0)
    Blocked(T),
}
impl<T> PopError<T> {
    fn map_inner<U, F: Fn(T) -> U>(self, f: F) -> PopError<U> {
        match self {
            Self::Empty(inner) => PopError::Empty(f(inner)),
            Self::Blocked(inner) => PopError::Blocked(f(inner)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{PopError, Tree};
    #[test]
    fn creates_single() {
        let mut t = Tree::new();
        let root_id = t.root_id();
        // item
        const N: usize = 10;
        for i in 0..N {
            t.push_item(&root_id, i).expect("root exists");
        }
        for i in 0..N {
            assert_eq!(t.pop_item_from(&root_id).expect("root exists"), Ok(i));
        }
        assert_eq!(
            t.pop_item_from(&root_id).expect("root exists"),
            Err(PopError::Empty(root_id.clone()))
        );
        // filter
        t.set_filter(&root_id, String::from("my root"))
            .expect("root exists");
        assert_eq!(
            t.get_filter(&root_id).expect("root exists"),
            &String::from("my root")
        );
    }
    #[test]
    fn two_nodes() {
        let mut t = Tree::new();
        let root_id = t.root_id();
        //
        let child_id = t.add_child(&root_id, None).expect("root exists");
        // filter
        t.set_filter(&child_id, String::from("child_filter"))
            .expect("child exists");
        t.set_filter(&root_id, String::from("root_filter"))
            .expect("root exists");
        // item
        const N: usize = 5;
        for i in 0..N {
            t.push_item(&child_id, i).expect("child exists");
            t.push_item(&root_id, i + 500).expect("root exists");
        }
        for i in 0..N {
            assert_eq!(t.pop_item_from(&child_id).expect("child exists"), Ok(i));
            assert_eq!(t.pop_item_from(&root_id).expect("root exists"), Ok(i + 500));
        }
        assert_eq!(
            t.pop_item_from(&child_id).expect("child exists"),
            Err(PopError::Empty(child_id))
        );
        assert_eq!(
            t.pop_item_from(&root_id).expect("root exists"),
            Err(PopError::Blocked(root_id))
        );
    }
    #[test]
    fn node_pop_chain() {
        let mut t: Tree<_, ()> = Tree::new();
        let root_id = t.root_id();
        //
        let child1 = t.add_child(&root_id, None).expect("root exists");
        let child2 = t.add_child(&child1, None).expect("child1 exists");
        // fill child2
        for i in 0..10 {
            t.push_item(&child2, i).expect("child2 exists");
        }
        // verify child2 pop
        assert_eq!(t.pop_item_from(&child2).expect("child2 exists"), Ok(0));
        assert_eq!(t.pop_item_from(&child2).expect("child2 exists"), Ok(1));
        // verify child1 not popping
        assert_eq!(
            t.pop_item_from(&child1).expect("child2 exists"),
            Err(PopError::Blocked(child1.clone()))
        );
        // allow child1 <- child2
        t.set_weight(&child2, 1).expect("child2 exists");
        // verify child1 chain from child2
        assert_eq!(t.pop_item_from(&child1).expect("child2 exists"), Ok(2));
        assert_eq!(t.pop_item_from(&child1).expect("child2 exists"), Ok(3));
    }
}
