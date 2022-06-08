// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use std::collections::VecDeque;

use crate::{
    error::{InvalidNodePath, RemoveError},
    id::{NodePathElem, Sequence, SequenceSource},
    order,
    weight_vec::{self, OrderVec},
    Weight,
};

/// Element in the [`Tree`](`crate::Tree`)
#[derive(Clone)]
pub struct Node<T, F> {
    pub(crate) children: Children<T, F>,
    /// Items queue polled from child nodes/items
    queue: VecDeque<T>,
    /// Optional filter qualifier
    pub filter: Option<F>,
    pub(crate) sequence: Sequence,
}
impl<T, F> Node<T, F> {
    /// Sets the [`OrderType`](`order::Type`)
    pub fn set_order_type(&mut self, order: order::Type) {
        match &mut self.children {
            Children::Chain(chain) => chain.nodes.set_order(order),
            Children::Items(items) => items.set_order(order),
        }
    }
    /// Gets the [`OrderType`](`order::Type`)
    pub fn get_order_type(&mut self) -> order::Type {
        match &self.children {
            Children::Chain(chain) => chain.nodes.get_order_type(),
            Children::Items(items) => items.get_order_type(),
        }
    }
    /// Appends an item to the queue
    pub fn push_item(&mut self, item: T) {
        self.queue.push_back(item);
    }
    /// Pops an item from self-queue only
    ///
    /// See: [`pop_item`] or [`pop_item_queued`]
    pub(crate) fn pop_only_from_self(&mut self) -> Option<T> {
        self.queue.pop_front()
    }
    /// Pops an item from child node queues only (ignores items-leaf nodes)
    // ///
    // /// See: [`pop_item`] for including items-leaf items for when `T: Copy`
    pub fn pop_item_queued(&mut self) -> Option<T> {
        self.pop_only_from_self()
            .or_else(|| match &mut self.children {
                Children::Chain(chain) => chain.find_next_item_queued(),
                Children::Items(_) => None,
            })
    }
    /// Overwrites children with the specified items (equally-weighted)
    pub fn set_child_items_uniform<I>(&mut self, items: I)
    where
        I: IntoIterator<Item = T>,
    {
        self.set_child_items(items.into_iter().map(|x| (1, x)));
    }
    /// Overwrites children with the specified items
    pub fn set_child_items<I>(&mut self, items: I)
    where
        I: IntoIterator<Item = (Weight, T)>,
    {
        let order = self.get_order_type();
        self.children = OrderVec::from((order, items)).into();
    }
    /// Returns the number of child nodes
    #[must_use]
    pub fn child_nodes_len(&self) -> usize {
        self.children.len_nodes()
    }
}
impl<T: Copy, F> Node<T, F> {
    /// Removes items from node queues, and finally copies from items-leaf node
    pub(crate) fn pop_item(&mut self) -> Option<T> {
        self.pop_only_from_self()
            .or_else(|| match &mut self.children {
                Children::Chain(chain) => chain.find_next_item(),
                Children::Items(items) => items.next().copied(),
            })
    }
}
impl<T, F> SequenceSource for Node<T, F> {
    fn sequence(&self) -> Sequence {
        self.sequence
    }
}

impl<T, F> std::fmt::Debug for Node<T, F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO
        f.debug_struct("Node").finish()
    }
}

#[derive(Clone)]
pub(crate) enum Children<T, F> {
    Chain(Chain<T, F>),
    Items(OrderVec<T>),
}
impl<T, F> Children<T, F> {
    /// Sum the count of all nodes, including `self`
    pub(crate) fn sum_node_count(&self) -> usize {
        let child_count = match self {
            Self::Chain(chain) => chain.sum_child_node_count(),
            Self::Items(_) => 0,
        };
        child_count + 1
    }
    pub(crate) fn len_nodes(&self) -> usize {
        match self {
            Self::Chain(chain) => chain.nodes.len(),
            Self::Items(_) => 0,
        }
    }
    pub(crate) fn is_empty_nodes(&self) -> bool {
        match self {
            Self::Chain(chain) => chain.nodes.is_empty(),
            Self::Items(_) => true,
        }
    }
}
impl<T, F> From<Chain<T, F>> for Children<T, F> {
    fn from(chain: Chain<T, F>) -> Self {
        Self::Chain(chain)
    }
}
impl<T, F> From<OrderVec<T>> for Children<T, F> {
    fn from(items: OrderVec<T>) -> Self {
        Self::Items(items)
    }
}

pub type RemoveResult<T, F, U> = Result<(Weight, NodeInfoIntrinsic<T, F>), RemoveError<U>>;

#[derive(Clone)]
pub(crate) struct Chain<T, F> {
    pub nodes: OrderVec<Node<T, F>>,
}
impl<T, F> Chain<T, F> {
    pub(crate) fn new(order: order::Type) -> Self {
        Self {
            nodes: OrderVec::new(order),
        }
    }
    pub(crate) fn sum_child_node_count(&self) -> usize {
        self.nodes
            .iter()
            .map(|(_, node)| node.children.sum_node_count())
            .sum()
    }
    pub(crate) fn get_child_entry_mut(
        &mut self,
        id_elems: &[NodePathElem],
    ) -> Result<weight_vec::RefMutElem<'_, '_, Node<T, F>>, InvalidNodePath> {
        if let Some((&this_idx, remainder)) = id_elems.split_first() {
            let child_ref = self
                .nodes
                .ref_mut()
                .into_elem_ref(this_idx)
                .or(Err(id_elems))?;
            if remainder.is_empty() {
                Ok(child_ref)
            } else {
                match &mut child_ref.1.children {
                    Children::Chain(chain) => chain.get_child_entry_mut(remainder),
                    Children::Items(_) => Err(id_elems.into()),
                }
            }
        } else {
            Err(id_elems.into())
        }
    }
    pub(crate) fn remove_child<S: SequenceSource>(
        &mut self,
        path_elem: NodePathElem,
        sequence: &S,
    ) -> Result<RemoveResult<T, F, NodePathElem>, NodePathElem> {
        let (_, child) = self.nodes.get(path_elem).ok_or(path_elem)?;
        let remove_result = if child.children.is_empty_nodes() {
            let child_sequence = child.sequence();
            if child_sequence == sequence.sequence() {
                Ok(self
                    .nodes
                    .ref_mut()
                    .remove(path_elem)
                    .map(|(weight, node)| {
                        let (child_weights, _seq, info_intrinsic) = NodeInfo::from(node).into();
                        assert!(child_weights.is_empty());
                        (weight, info_intrinsic)
                    })
                    .expect("node at index exists just after getting some"))
            } else {
                Err(RemoveError::SequenceMismatch(path_elem, child_sequence))
            }
        } else {
            Err(RemoveError::NonEmpty(path_elem))
        };
        Ok(remove_result)
    }
    pub(crate) fn find_next_item_queued(&mut self) -> Option<T> {
        self.find_next_item_using_fn(Node::pop_item_queued)
    }
    fn find_next_item_using_fn<U>(&mut self, mut pop_fn: U) -> Option<T>
    where
        U: FnMut(&mut Node<T, F>) -> Option<T>,
    {
        const INVALID_INDEX: &str = "valid index from next_index";
        let first_node_index = self.nodes.next_index()?;
        let first_node = self
            .nodes
            .get_elem_mut(first_node_index)
            .expect(INVALID_INDEX);
        if let Some(item) = pop_fn(first_node) {
            // fast path
            Some(item)
        } else {
            // search
            let mut nodes_visited = vec![Some(()); self.nodes.len()];
            nodes_visited[first_node_index].take();
            loop {
                let node_index = self.nodes.next_index()?;
                // base case - return `None` if node already visited
                nodes_visited
                    .get_mut(node_index)
                    .expect(INVALID_INDEX)
                    .take()?;
                //
                let node = self.nodes.get_elem_mut(node_index).expect(INVALID_INDEX);
                if let Some(item) = pop_fn(node) {
                    break Some(item);
                }
            }
        }
    }
}
impl<T: Copy, F> Chain<T, F> {
    pub(crate) fn find_next_item(&mut self) -> Option<T> {
        self.find_next_item_using_fn(Node::pop_item)
    }
}

pub(crate) use meta::SequenceCounter;

use self::meta::{NodeInfo, NodeInfoIntrinsic};
pub(crate) mod meta {
    use std::collections::VecDeque;

    use serde::{Deserialize, Serialize};

    use crate::{
        id::{self, ty, NodeId, Sequence},
        node::{Chain, Children},
        order,
        weight_vec::OrderVec,
        Weight,
    };

    use super::Node;
    /// Serializable representation of a filter/queue/merge element in the [`Tree`](`crate::Tree`)
    #[must_use]
    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub(crate) struct NodeInfo<T, F>(Vec<Weight>, Sequence, NodeInfoIntrinsic<T, F>);
    impl<T, F> From<NodeInfo<T, F>> for (Vec<Weight>, Sequence, NodeInfoIntrinsic<T, F>) {
        fn from(node_info: NodeInfo<T, F>) -> Self {
            (node_info.0, node_info.1, node_info.2)
        }
    }
    impl<T, F> From<Node<T, F>> for NodeInfo<T, F> {
        fn from(node: Node<T, F>) -> Self {
            let Node {
                children,
                queue,
                filter,
                sequence,
            } = node;
            match children {
                Children::Chain(Chain { nodes }) => {
                    let (order, (weights, _nodes)) = nodes.into_parts();
                    let info_intrinsic = NodeInfoIntrinsic::Chain {
                        queue,
                        filter,
                        order,
                    };
                    Self(weights, sequence, info_intrinsic)
                }
                Children::Items(items) => {
                    let (order, (weights, items)) = items.into_parts();
                    let info_intrinsic = NodeInfoIntrinsic::Items {
                        items,
                        filter,
                        order,
                    };
                    Self(weights, sequence, info_intrinsic)
                }
            }
        }
    }
    /// Intrinsic description of a Node
    #[must_use]
    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(untagged)]
    pub enum NodeInfoIntrinsic<T, F> {
        /// Node containing nodes as children
        Chain {
            /// Items queue polled from child nodes
            queue: VecDeque<T>,
            /// Filtering value
            filter: Option<F>,
            // TODO
            // /// Minimum number of items to retain in queue, beyond which [`PopError::NeedsPush`] is raised
            // pub retain_count: usize,
            /// Ordering type for child nodes
            order: order::Type,
        },
        /// Node containing only child items (no child nodes)
        Items {
            /// Items
            items: Vec<T>,
            /// Filtering value
            filter: Option<F>,
            /// Ordering type for child items
            order: order::Type,
        },
    }
    impl<T, F> Default for NodeInfoIntrinsic<T, F> {
        fn default() -> Self {
            Self::Chain {
                queue: VecDeque::default(),
                filter: Option::default(),
                order: order::Type::default(),
            }
        }
    }
    impl<T, F> NodeInfoIntrinsic<T, F> {
        pub(crate) fn construct_root(self) -> (Node<T, F>, SequenceCounter) {
            const ROOT_ID: NodeId<ty::Root> = id::ROOT;
            let root = self.make_node(ROOT_ID.sequence());
            let counter = SequenceCounter::new(&ROOT_ID);
            (root, counter)
        }
        pub(crate) fn construct(self, counter: &mut SequenceCounter) -> Node<T, F> {
            self.make_node(counter.next())
        }
        fn make_node(self, sequence: Sequence) -> Node<T, F> {
            match self {
                Self::Chain {
                    queue,
                    filter,
                    order,
                } => Node {
                    children: Chain::new(order).into(),
                    queue,
                    filter,
                    sequence,
                },
                Self::Items {
                    items,
                    filter,
                    order,
                } => Node {
                    children: OrderVec::from((order, items.into_iter().map(|item| (1, item))))
                        .into(),
                    queue: VecDeque::new(),
                    filter,
                    sequence,
                },
            }
        }
    }

    /// Counter for a `Sequence`
    #[derive(Debug)]
    pub(crate) struct SequenceCounter(Sequence);
    impl SequenceCounter {
        fn new(from_id: &NodeId<ty::Root>) -> Self {
            Self(from_id.sequence())
        }
        /// Returns the next Sequence value in the counter
        fn next(&mut self) -> Sequence {
            self.0 += 1;
            self.0
        }
    }
}
