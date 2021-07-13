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

use node_id::{NodeId, NodeIdElem};
mod node_id {
    /// Representation for Root ID
    pub const ROOT: NodeId = NodeId(vec![]);

    #[allow(clippy::module_name_repetitions)]
    /// Element of a [`NodeId`]
    pub type NodeIdElem = usize;

    /// Identifier for a Node in the [`Tree`]
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct NodeId(Vec<NodeIdElem>);
    impl NodeId {
        /// Appends an element to the ID
        pub fn extend(&self, next: NodeIdElem) -> NodeId {
            let mut parts = self.0.clone();
            parts.push(next);
            Self(parts)
        }
        /// Returns the parent ID (if it exists)
        pub fn parent(&self) -> Option<NodeId> {
            if self.0.is_empty() {
                None
            } else {
                let mut parts = self.0.clone();
                parts.pop();
                Some(Self(parts))
            }
        }
        pub fn first_elem(&self) -> Option<NodeIdElem> {
            self.0.get(0).copied()
        }
    }
    impl From<Vec<NodeIdElem>> for NodeId {
        fn from(elems: Vec<NodeIdElem>) -> Self {
            Self(elems)
        }
    }
    impl<'a> From<&'a NodeId> for &'a [NodeIdElem] {
        fn from(node_id: &'a NodeId) -> Self {
            node_id.0.as_slice()
        }
    }
}

/// Numeric type for weighting nodes in the [`Tree`], used by to fuel [`MergeOrder`] algorithms
pub type Weight = u32;

/// Error for an invalid [`NodeId`]
#[derive(Debug)]
pub struct InvalidNodeId(NodeId);
impl From<&[NodeIdElem]> for InvalidNodeId {
    fn from(node_id: &[NodeIdElem]) -> Self {
        Self(node_id.to_vec().into())
    }
}

struct Tree<T, F> {
    root: Node<T, F>,
}
impl<T, F> Tree<T, F>
where
    F: Default,
{
    pub fn new() -> Self {
        let root = Node::default();
        Tree { root }
    }
    #[allow(clippy::unused_self)]
    pub fn root_id(&self) -> NodeId {
        node_id::ROOT
    }
    fn get_node(&self, node_id: &NodeId) -> Result<&Node<T, F>, InvalidNodeId> {
        self.root.get_child(node_id.into())
    }
    fn get_node_mut(&mut self, node_id: &NodeId) -> Result<&mut Node<T, F>, InvalidNodeId> {
        self.root.get_child_mut(node_id.into())
    }
    pub fn add_child(
        &mut self,
        node_id: &NodeId,
        weight: Option<Weight>,
    ) -> Result<NodeId, InvalidNodeId> {
        let parent = self.get_node_mut(node_id)?;
        Ok(parent.add_child(node_id, weight))
    }
    pub fn set_weight(&mut self, node_id: &NodeId, weight: Weight) -> Result<(), InvalidNodeId> {
        self.root.set_weight(node_id.into(), weight)
    }
    pub fn get_filter(&self, node_id: &NodeId) -> Result<&F, InvalidNodeId> {
        let node = self.get_node(node_id)?;
        Ok(&node.filter)
    }
    pub fn set_filter(&mut self, node_id: &NodeId, filter: F) -> Result<(), InvalidNodeId> {
        let node = self.get_node_mut(node_id)?;
        node.filter = filter;
        Ok(())
    }
    pub fn push_item(&mut self, node_id: &NodeId, item: T) -> Result<(), InvalidNodeId> {
        let node = self.get_node_mut(node_id)?;
        node.queue.push_back(item);
        Ok(())
    }
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

mod order {
    use super::Weight;

    pub enum State {
        Empty(Type),
        State(Box<dyn Order>),
    }
    impl State {
        pub fn get_type(&self) -> Type {
            match self {
                Self::Empty(ty) => *ty,
                Self::State(order) => order.get_type(),
            }
        }
    }
    impl From<Type> for State {
        fn from(ty: Type) -> Self {
            Self::Empty(ty)
        }
    }
    impl PartialEq for State {
        fn eq(&self, other: &State) -> bool {
            self.get_type() == other.get_type()
        }
    }
    impl Eq for State {}
    impl std::fmt::Debug for State {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            let variant = match self {
                Self::Empty(_) => "Empty",
                Self::State(_) => "State",
            };
            let ty = self.get_type();
            write!(f, "State::{}({:?})", variant, ty)
        }
    }

    /// Order of picking nodes from children nodes, given the node [`Weight`]s.
    #[allow(clippy::module_name_repetitions)]
    #[derive(Debug, Eq, PartialEq, Clone, Copy)]
    pub enum Type {
        /// Picks [`Weight`] items from one node before moving to the next node
        InOrder,
    }
    impl Type {
        fn instantiate(self, weights: &[Weight]) -> Box<dyn Order> {
            match self {
                Type::InOrder => Box::new(InOrderState::new(weights)),
            }
        }
    }

    pub trait Order {
        fn get_type(&self) -> Type;
        fn resize_to(&mut self, weights: &[Weight]);
        fn get_weights(&self) -> &[Weight];
        fn inner_next(&mut self) -> Option<usize>;
        fn next(&mut self, weights: &[Weight]) -> Option<usize> {
            if self.get_weights() != weights {
                self.resize_to(weights);
            }
            self.inner_next()
        }
    }

    struct InOrderState {
        weights: Vec<Weight>,
        index_remaining: Option<(usize, Weight)>,
    }
    impl InOrderState {
        fn new(weights: &[Weight]) -> Self {
            let mut this = Self {
                weights: vec![],
                index_remaining: None,
            };
            this.resize_to(weights);
            this
        }
    }
    impl Order for InOrderState {
        fn get_type(&self) -> Type {
            Type::InOrder
        }
        fn resize_to(&mut self, weights: &[Weight]) {
            self.weights = weights.to_vec();
            self.index_remaining = None;
        }
        fn get_weights(&self) -> &[Weight] {
            &self.weights
        }
        fn inner_next(&mut self) -> Option<usize> {
            let filter_nonzero_weight = |(index, &weight)| {
                if weight > 0 {
                    Some((index, weight - 1))
                } else {
                    None
                }
            };
            self.index_remaining = self
                .index_remaining
                .and_then(|(index, weight)| {
                    if weight > 0 {
                        Some((index, weight - 1))
                    } else {
                        let index = index + 1;
                        // search Tail then Head for first non-zero weight
                        let tail = self.weights.iter().enumerate().skip(index);
                        let head = self.weights.iter().enumerate();
                        tail.chain(head).find_map(filter_nonzero_weight)
                    }
                })
                .or_else(|| {
                    // find first index of non-zero weight
                    self.weights
                        .iter()
                        .enumerate()
                        .find_map(filter_nonzero_weight)
                });
            // next index
            self.index_remaining.map(|(index, _)| index)
        }
    }
    #[cfg(test)]
    mod tests {
        use super::Type;
        fn check_simple(ty: Type) {
            let weights = &[1];
            let mut s = ty.instantiate(weights);
            for _ in 0..100 {
                assert_eq!(s.next(weights), Some(0));
            }
        }
        fn check_blocked(ty: Type) {
            let weights = &[0];
            let mut s = ty.instantiate(weights);
            for _ in 0..100 {
                assert_eq!(s.next(weights), None);
            }
        }
        // Type::InOrder
        #[test]
        fn in_order_simple() {
            check_simple(Type::InOrder);
        }
        #[test]
        fn in_order_blocked() {
            check_blocked(Type::InOrder);
        }
        #[test]
        fn in_order_longer() {
            let weights = &[1, 2, 2, 3, 0, 5];
            let mut s = Type::InOrder.instantiate(weights);
            for _ in 0..100 {
                for (index, &weight) in weights.iter().enumerate() {
                    for _ in 0..weight {
                        assert_eq!(s.next(weights), Some(index));
                        //
                        // let value = s.next(weights);
                        // let expected = Some(index);
                        // assert_eq!(value, expected);
                        // println!("{:?} = {:?} ??", value, expected);
                    }
                }
            }
        }
    }
}

pub use node::Node;
mod node {
    use super::{order, InvalidNodeId, NodeId, NodeIdElem, PopError, Weight};
    use std::collections::VecDeque;

    type Child<T, F> = (Weight, Node<T, F>);

    /// Internal representation of a filter/queue/merge element in the [`Tree`]
    #[must_use]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Node<T, F> {
        /// Items queue
        pub queue: VecDeque<T>,
        /// Filtering value
        pub filter: F,
        children: Vec<(Weight, Node<T, F>)>,
        order: order::State,
    }
    impl<T, F> Default for Node<T, F>
    where
        F: Default,
    {
        fn default() -> Self {
            Self {
                queue: VecDeque::new(),
                filter: F::default(),
                children: vec![],
                order: order::Type::InOrder.into(),
            }
        }
    }
    impl<T, F> Node<T, F>
    where
        F: Default,
    {
        /// Adds a child to the specified `Node`, with an optional `Weight`
        pub fn add_child(&mut self, node_id: &NodeId, weight: Option<Weight>) -> NodeId {
            let weight = weight.unwrap_or(0);
            let new_child = (weight, Node::default());
            let child_part = {
                let child_part = self.children.len() as NodeIdElem;
                // push AFTER recording length ^
                self.children.push(new_child);
                child_part
            };
            self.clear_order_state();
            // return new NodeId
            node_id.extend(child_part)
        }
    }
    impl<T, F> Node<T, F> {
        /// Returns the child `Node` at the specified ID elements path
        ///
        /// # Errors
        /// Returns an error if the specified `NodeId` does not point to a valid node
        ///
        pub fn get_child(&self, id_elems: &[NodeIdElem]) -> Result<&Node<T, F>, InvalidNodeId> {
            if id_elems.is_empty() {
                Ok(self)
            } else {
                self.get_child_entry(id_elems).map(|(_, child)| child)
            }
        }
        /// Returns the child `Node` at the specified ID elements path
        ///
        /// # Errors
        /// Returns an error if the specified `NodeId` does not point to a valid node
        ///
        pub fn get_child_mut(
            &mut self,
            id_elems: &[NodeIdElem],
        ) -> Result<&mut Node<T, F>, InvalidNodeId> {
            if id_elems.is_empty() {
                Ok(self)
            } else {
                self.get_child_entry_mut(id_elems).map(|(_, child)| child)
            }
        }
        fn get_child_entry(
            &self,
            id_elems: &[NodeIdElem],
        ) -> Result<&(Weight, Node<T, F>), InvalidNodeId> {
            if let Some((&this_idx, remainder)) = id_elems.split_first() {
                let child = self.children.get(this_idx).ok_or(id_elems)?;
                if remainder.is_empty() {
                    Ok(child)
                } else {
                    let (_, child_node) = child;
                    child_node.get_child_entry(remainder)
                }
            } else {
                Err(id_elems.into())
            }
        }
        fn get_child_entry_mut(
            &mut self,
            id_elems: &[NodeIdElem],
        ) -> Result<&mut (Weight, Node<T, F>), InvalidNodeId> {
            if let Some((&this_idx, remainder)) = id_elems.split_first() {
                let child = self.children.get_mut(this_idx).ok_or(id_elems)?;
                if remainder.is_empty() {
                    Ok(child)
                } else {
                    let (_, child_node) = child;
                    child_node.get_child_entry_mut(remainder)
                }
            } else {
                Err(id_elems.into())
            }
        }
        fn clear_order_state(&mut self) {
            self.order = order::State::Empty(self.order.get_type());
        }
        /// Sets the weight of the specified `Node`
        ///
        /// # Errors
        /// Returns an error if the specified `NodeId` does not point to a valid node
        ///
        pub fn set_weight(
            &mut self,
            node_id: &[NodeIdElem],
            weight: Weight,
        ) -> Result<(), InvalidNodeId> {
            let (c_weight, _) = self.get_child_entry_mut(node_id)?;
            *c_weight = weight;
            self.clear_order_state();
            Ok(())
        }
        /// Attempts to pop the next item
        ///
        /// # Errors
        /// Returns an error if the pop operation fails
        ///
        pub fn pop_item(&mut self) -> Result<T, PopError<()>> {
            #[allow(clippy::option_if_let_else)]
            if let Some(item) = self.queue.pop_front() {
                Ok(item)
            } else {
                // TODO
                Err(PopError::Empty(()))
            }
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
            Err(PopError::Empty(root_id))
        );
    }
    #[test]
    #[ignore] //TODO: implement this
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
            Err(PopError::Empty(child1.clone()))
        );
        // allow child1 <- child2
        t.set_weight(&child2, 1).expect("child2 exists");
        // verify child1 chain from child2
        assert_eq!(t.pop_item_from(&child1).expect("child2 exists"), Ok(2));
        assert_eq!(t.pop_item_from(&child1).expect("child2 exists"), Ok(3));
    }
}
