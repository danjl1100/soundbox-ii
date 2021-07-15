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

use node::Node;
mod node;

pub use id::{NodeId, NodePath};
use id::{NodePathElem, Sequence, SequenceSource};
mod id;

pub use order::Type as OrderType;
mod order;

/// Numeric type for weighting nodes in the [`Tree`], used by to fuel [`OrderType`] algorithms
pub type Weight = u32;

/// Error for an invalid [`NodeId`] path
#[derive(Debug, PartialEq, Eq)]
pub struct InvalidNodePath(NodePath);
impl From<&[NodePathElem]> for InvalidNodePath {
    fn from(node_id: &[NodePathElem]) -> Self {
        Self(node_id.to_vec().into())
    }
}

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

/// Error from the [`Tree::pop_item_from`] operation
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

/// Error from the [`Tree::remove_node`] operation
/// Generic parameters set to:
/// - `T`=[`NodePath`] is the path for parent node
/// - `U`=[`NodePath`] is the path for children of the parent node
/// - `V`=[`NodeId`] is the id for the target removal node
///
pub type RemoveError = RemoveErrorGeneric<NodePath, NodePath, NodeId>;
pub(crate) type RemoveErrorInner = RemoveErrorGeneric<(), NodePathElem, ()>;
/// Error from the [`Tree::remove_node`] operation.
/// Generic type parameters used internally within nodes:
/// - `T` is the path for parent node
/// - `U` is the path for children of the parent node
/// - `V` is the [`NodeId`] for the target removal node
///
#[derive(Debug, PartialEq, Eq)]
pub enum RemoveErrorGeneric<T, U, V> {
    /// No node matching the [`NodeId`]
    Invalid(T),
    /// Node matching the [`NodeId`] path has a different sequence (e.g. node paths changed)
    SequenceMismatch(V, Sequence),
    /// Root node cannot be removed
    Root(T),
    /// Node has outstanding children
    NonEmpty(T, Vec<U>),
}
impl RemoveErrorInner {
    fn attach_id(self, node_id: &NodeId) -> RemoveError {
        let node_id = node_id.clone();
        match self {
            Self::Invalid(()) => RemoveError::Invalid(node_id.into()),
            Self::SequenceMismatch((), sequence) => {
                RemoveError::SequenceMismatch(node_id, sequence)
            }
            Self::Root(()) => RemoveError::Root(node_id.into()),
            Self::NonEmpty((), children) => {
                let children = children
                    .into_iter()
                    .map(|elem| node_id.extend(elem))
                    .collect();
                RemoveError::NonEmpty(node_id.into(), children)
            }
        }
    }
}
impl<U, V> From<InvalidNodePath> for RemoveErrorGeneric<NodePath, U, V> {
    fn from(other: InvalidNodePath) -> Self {
        Self::Invalid(other.0)
    }
}

#[cfg(test)]
mod tests {
    use super::{PopError, RemoveError, SequenceSource, Tree};
    #[test]
    fn creates_single() {
        let mut t = Tree::new();
        let root = t.root_id();
        // item
        const N: usize = 10;
        for i in 0..N {
            t.push_item(&root, i).expect("root exists");
        }
        for i in 0..N {
            assert_eq!(t.pop_item_from(&root).expect("root exists"), Ok(i));
        }
        assert_eq!(
            t.pop_item_from(&root).expect("root exists"),
            Err(PopError::Empty(root.clone()))
        );
        // filter
        t.set_filter(&root, String::from("my root"))
            .expect("root exists");
        assert_eq!(
            t.get_filter(&root).expect("root exists"),
            &String::from("my root")
        );
    }
    #[test]
    fn two_nodes() {
        let mut t = Tree::new();
        let root = t.root_id();
        //
        let child = t.add_child(&root, None).expect("root exists");
        // filter
        t.set_filter(&child, String::from("child_filter"))
            .expect("child exists");
        t.set_filter(&root, String::from("root_filter"))
            .expect("root exists");
        // item
        const N: usize = 5;
        for i in 0..N {
            t.push_item(&child, i).expect("child exists");
            t.push_item(&root, i + 500).expect("root exists");
        }
        for i in 0..N {
            assert_eq!(t.pop_item_from(&child).expect("child exists"), Ok(i));
            assert_eq!(t.pop_item_from(&root).expect("root exists"), Ok(i + 500));
        }
        assert_eq!(
            t.pop_item_from(&child).expect("child exists"),
            Err(PopError::Empty(child))
        );
        assert_eq!(
            t.pop_item_from(&root).expect("root exists"),
            Err(PopError::Blocked(root))
        );
    }
    #[test]
    fn node_pop_chain() {
        let mut t: Tree<_, ()> = Tree::new();
        let root = t.root_id();
        //
        let child1 = t.add_child(&root, None).expect("root exists");
        let child2 = t.add_child(&child1, None).expect("child1 exists");
        // fill child2
        for i in 0..4 {
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
        assert_eq!(
            t.pop_item_from(&child1).expect("child2 exists"),
            Err(PopError::Empty(child1))
        );
    }
    #[test]
    fn node_removal() {
        let mut t: Tree<_, ()> = Tree::new();
        let root = t.root_id();
        // \ root
        // ---\ base
        //    |--  child1
        //    |--  child2
        //    |--  child3
        //    |--\ child4
        //       |--  child4_child
        //    |--  child5
        let base = t.add_child(&root, None).expect("root exists");
        let child1 = t.add_child(&base, None).expect("base exists");
        let child2 = t.add_child(&base, None).expect("base exists");
        let child3 = t.add_child(&base, None).expect("base exists");
        let child4 = t.add_child(&base, None).expect("base exists");
        let child5 = t.add_child(&base, None).expect("base exists");
        let child4_child = t.add_child(&child4, None).expect("child4 exists");
        // fill child4
        for i in 0..10 {
            t.push_item(&child4, i).expect("child4 exists");
        }
        // verify root pop
        t.set_weight(&base, 1).expect("base exists");
        t.set_weight(&child4, 1).expect("child4 exists");
        assert_eq!(t.pop_item_from(&root).expect("root exists"), Ok(0));
        assert_eq!(t.pop_item_from(&root).expect("root exists"), Ok(1));
        // fails - remove root
        assert_eq!(
            t.remove_node(&root),
            Err(RemoveError::Root(root.clone().into()))
        );
        // fails - remove base
        assert_eq!(
            t.remove_node(&base),
            Err(RemoveError::NonEmpty(
                base.clone().into(),
                vec![
                    child1.clone().into(),
                    child2.clone().into(),
                    child3.clone().into(),
                    child4.clone().into(),
                    child5.clone().into(),
                ]
            ))
        );
        // fails - remove child4
        assert_eq!(
            t.remove_node(&child4),
            Err(RemoveError::NonEmpty(
                child4.clone().into(),
                vec![child4_child.clone().into()]
            ))
        );
        // success - remove child4_child, then child4
        assert_eq!(t.remove_node(&child4_child), Ok(()));
        assert_eq!(t.remove_node(&child4), Ok(()));
        // fails - remove child4 AGAIN
        assert_eq!(
            t.remove_node(&child4),
            Err(RemoveError::SequenceMismatch(child4, child5.sequence()))
        );
        // verify root pop empty
        assert_eq!(
            t.pop_item_from(&root).expect("root exists"),
            Err(PopError::Blocked(root))
        );
    }
}
