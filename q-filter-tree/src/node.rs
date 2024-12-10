// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use std::{borrow::Cow, collections::VecDeque};

use crate::{
    error::{InvalidNodePath, RemoveError},
    id::{NodePathElem, Sequence, SequenceSource},
    order,
    weight_vec::{self, OrderVec},
    SequenceAndItem, Weight,
};

/// Element in the [`Tree`](`crate::Tree`)
#[derive(Clone)]
pub struct Node<T, F> {
    pub(crate) children: Children<T, F>,
    /// Items queue polled from child nodes/items
    queue: VecDeque<SequenceAndItem<T>>,
    /// Number of elements to automatically pre-fill into the queue
    queue_prefill_len: usize,
    /// Filter qualifier
    pub filter: F,
    sequence: Sequence,
}
impl<T, F> Node<T, F> {
    /// Sets the [`OrderType`](`order::Type`)
    pub fn set_order_type(&mut self, order: order::Type) -> order::Type {
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
    /// Appends an item to the queue, marked as originating from this node
    pub fn push_item(&mut self, item: T) {
        let seq = self.sequence();
        self.push_seq_item(SequenceAndItem::new(seq, item));
    }
    fn push_seq_item(&mut self, seq_item: SequenceAndItem<T>) {
        self.queue.push_back(seq_item);
    }
    /// Returns a the length of the queue
    pub fn queue_len(&self) -> usize {
        self.queue.len()
    }
    /// Returns an iterator over the queue
    pub fn queue_iter(&self) -> impl Iterator<Item = &SequenceAndItem<T>> {
        self.queue.iter()
    }
}
impl<T: Clone, F> Node<T, F> {
    /// Removes the specified item from the queue
    ///
    /// # Errors
    /// Returns an error of the current length, if the specified index is out of bounds
    pub fn try_queue_remove(&mut self, index: usize) -> Result<Option<SequenceAndItem<T>>, usize> {
        let queue_len = self.queue.len();
        if index < queue_len {
            let removed = self.queue.remove(index);
            self.queue_prefill(false);
            Ok(removed)
        } else {
            Err(queue_len)
        }
    }
    /// Sets the number of elements to automatically pre-fill into the queue
    pub fn set_queue_prefill_len(&mut self, new_len: usize) {
        self.queue_prefill_len = new_len;
        self.queue_prefill(false);
    }
    /// Verifies the queue pre-fill is updated
    pub(crate) fn update_queue_prefill(&mut self) {
        self.queue_prefill(false);
    }
    // TODO define criteria for when to speculatively `prefill` parent nodes
    //     e.g. on ANY change?  or only when more items are added?
    //    NOTE: `Node` methods have NO ACCESS to parent nodes.  Must be `Tree`-level function
    // Current workaround - client opt-in using TreeGuard to pre-fill nodes after mutating
    fn queue_prefill(&mut self, will_pop: bool) {
        let queue_prefill_len = match &self.children {
            Children::Chain(_) => self.queue_prefill_len,
            Children::Items(items) => self.queue_prefill_len.min(items.len()),
        };
        // NOTE: guarded to ensure `prefill = 0` allows Cow::Borrowed usage
        // (otherwise, `will_pop = true` unconditionally clones *every* element)
        if queue_prefill_len > 0 {
            let min_count = queue_prefill_len + if will_pop { 1 } else { 0 };
            while self.queue.len() < min_count {
                if let Some(popped) = self.inner_pop_item(Some(IgnoreQueue)) {
                    let popped = popped.map(Cow::into_owned);
                    self.push_seq_item(popped);
                } else {
                    break;
                }
            }
        }
    }
    /// Pops an item from child node queues (if available) then references items
    pub fn pop_item(&mut self) -> Option<SequenceAndItem<Cow<'_, T>>> {
        // NOTE only pre-fill at the side of the actual `pop_front`
        // self.queue_prefill(true);
        self.inner_pop_item(None)
    }
    fn inner_pop_item(
        &mut self,
        ignore_queue: Option<IgnoreQueue>,
    ) -> Option<SequenceAndItem<Cow<'_, T>>> {
        // 1) search Node for path to:
        //    1.a) Node X has queued item (Owned)
        //    1.b) Node X has an item to (Borrowed)
        self.find_reverse_path_to_pop(ignore_queue).map(|path| {
            // 2) retrieve the Owned or Borrowed item from the specified node
            self.pop_at_reverse_path(&path, ignore_queue)
        })
        // This all occurs in one `&mut self` function, so no intermediate access can occur.
    }
    fn find_reverse_path_to_pop(
        &mut self,
        ignore_queue: Option<IgnoreQueue>,
    ) -> Option<Vec<NodePathElem>> {
        const INVALID_INDEX: &str = "valid index from next_index";
        if self.queue.is_empty() || ignore_queue.is_some() {
            match &mut self.children {
                Children::Items(items) if items.is_empty() => None,
                Children::Items(_) => Some(vec![]),
                Children::Chain(chain) => {
                    let nodes = &mut chain.nodes;
                    let mut nodes_visited = vec![Some(()); nodes.len()];
                    loop {
                        let node_index = nodes.next_index()?;
                        // base case - return `None` if node already visited
                        nodes_visited
                            .get_mut(node_index)
                            .expect(INVALID_INDEX)
                            .take()?;
                        //
                        let node = nodes.get_elem_mut(node_index).expect(INVALID_INDEX);
                        if let Some(mut path) = node.find_reverse_path_to_pop(None) {
                            path.push(node_index);
                            break Some(path);
                        }
                    }
                }
            }
        } else {
            // queue has elems, path="THIS"
            Some(vec![])
        }
    }
    fn pop_at_reverse_path(
        &mut self,
        reverse_path: &[NodePathElem],
        ignore_queue: Option<IgnoreQueue>,
    ) -> SequenceAndItem<Cow<'_, T>> {
        if let Some((child_index, remainder)) = reverse_path.split_last() {
            match &mut self.children {
                Children::Chain(chain) => {
                    let child = chain
                        .nodes
                        .get_elem_mut(*child_index)
                        .expect("attempt to pop from a path descendent outside of range");
                    child.pop_at_reverse_path(remainder, None)
                }
                Children::Items(_) => {
                    unreachable!("attempt to pop from a path descendent of an items node")
                }
            }
        } else {
            let queued = ignore_queue
                .is_none()
                .then(|| {
                    self.queue_prefill(true);
                    self.queue.pop_front()
                })
                .flatten();
            let seq = self.sequence();
            match (queued, &mut self.children) {
                (Some(queued), _) => queued.map(Cow::Owned),
                (None, Children::Chain(_)) => {
                    unreachable!("attempt to pop at a chain node with no queue")
                }
                (None, Children::Items(items)) => {
                    let item = items
                        .next_item()
                        // Panic if the promised source of the item is `None` (logic error)
                        .expect("attempt to pop from items node with no value");
                    SequenceAndItem::new(seq, Cow::Borrowed(item))
                }
            }
        }
    }
}
impl<T, F> Node<T, F>
where
    T: PartialEq,
{
    /// Merges the child items with the specified items (attempting to maintain ordering state)
    pub fn merge_child_items_uniform<I>(&mut self, items: I)
    where
        I: IntoIterator<Item = T>,
    {
        self.merge_child_items(items.into_iter().map(|x| (1, x)));
    }
    /// Merges the child items with the specified items (attempting to maintain ordering state)
    #[allow(clippy::missing_panics_doc)] // bounds guaranteed by loop (needed for lending iterator)
    pub fn merge_child_items<I>(&mut self, items: I)
    where
        I: IntoIterator<Item = (Weight, T)>,
    {
        let mut items = items.into_iter();
        self.with_child_items(|child_items| {
            for index in 0..child_items.len() {
                if let Some((new_weight, new_item)) = items.next() {
                    let (mut weight_ref, item_ref) = child_items
                        .ref_mut()
                        .into_elem_ref(index)
                        .expect("child items yield element within the length bounds");
                    if new_weight != weight_ref.get_weight() {
                        weight_ref.set_weight(new_weight);
                    }
                    *item_ref = new_item;
                } else {
                    child_items.ref_mut().truncate(index);
                    break;
                }
            }
            child_items.extend(items.skip(child_items.len()));
            // TODO add test for this functionality
        });
    }
}
impl<T, F> Node<T, F> {
    /// Overwrites children with the specified items (equally-weighted)
    pub fn overwrite_child_items_uniform<I>(&mut self, items: I)
    where
        I: IntoIterator<Item = T>,
    {
        self.overwrite_child_items(items.into_iter().map(|x| (1, x)));
    }
    /// Overwrites children with the specified items
    pub fn overwrite_child_items<I>(&mut self, items: I)
    where
        I: IntoIterator<Item = (Weight, T)>,
    {
        let order = self.get_order_type();
        self.children = OrderVec::from((order, items)).into();
    }
    /// Allows mutable access to the items vector [`OrderVec`], deleting child nodes as needed.
    // TODO consider returning an error if the child nodes is not empty
    fn with_child_items<U, R>(&mut self, modify_fn: U) -> R
    where
        U: FnOnce(&mut OrderVec<T>) -> R,
    {
        match &mut self.children {
            Children::Chain(..) => {
                let order = self.get_order_type();
                let mut items = OrderVec::new(order);
                let result = modify_fn(&mut items);
                self.children = Children::Items(items);
                result
            }
            Children::Items(items) => modify_fn(items),
        }
    }
    /// Allows read-only access to the chain nodes (if any), or `None` for an item node
    ///
    /// NOTE: Mutable handles to nodes are obtained at tree-level via
    /// [`NodePathTyped::try_ref`](`crate::id::NodePathTyped::try_ref`)
    pub fn child_nodes(&self) -> Option<impl Iterator<Item = (Weight, &Node<T, F>)>> {
        match &self.children {
            Children::Chain(chain) => Some(chain.nodes.iter()),
            Children::Items(_) => None,
        }
    }

    /// Returns the number of child nodes
    #[must_use]
    pub fn child_nodes_len(&self) -> usize {
        self.children.len_nodes()
    }
    /// Returns the id-number sequence
    pub fn sequence_num(&self) -> Sequence {
        self.sequence
    }
}
impl<T, F> SequenceSource for Node<T, F> {
    fn sequence(&self) -> Sequence {
        self.sequence_num()
    }
}

impl<T, F> std::fmt::Debug for Node<T, F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO
        f.debug_struct("Node").finish()
    }
}

/// Marker for operations which shall ignore the queue
#[derive(Clone, Copy)]
struct IgnoreQueue;

#[derive(Clone)]
// see note in crate::order about boxing some Order variants
// (random state takes up a decent number of bytes)
#[allow(clippy::large_enum_variant)]
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
    pub(crate) fn get_nodes(&self) -> Option<&OrderVec<Node<T, F>>> {
        match self {
            Self::Chain(chain) => Some(&chain.nodes),
            Self::Items(_) => None,
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

/// Result for removing a node (when node is indeed found)
///
/// Generic for internal-use, returning from node-to-node during the removal
pub type RemoveResult<T, F, E> = Result<(Weight, NodeInfoIntrinsic<T, F>), E>;

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
    pub(crate) fn get_child_entry_shared(
        &self,
        id_elems: &[NodePathElem],
    ) -> Result<(Weight, &Node<T, F>), InvalidNodePath> {
        if let Some((&this_idx, remainder)) = id_elems.split_first() {
            let child_ref = self.nodes.get(this_idx).ok_or(id_elems)?;
            if remainder.is_empty() {
                Ok(child_ref)
            } else {
                match &child_ref.1.children {
                    Children::Chain(chain) => chain.get_child_entry_shared(remainder),
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
    ) -> Result<RemoveResult<T, F, RemoveError<NodePathElem>>, NodePathElem> {
        let (_, child) = self.nodes.get(path_elem).ok_or(path_elem)?;
        let (is_terminal, has_children) = {
            let nodes = child.children.get_nodes();
            let is_terminal = nodes.is_none();
            let has_children = nodes.is_some_and(|n| !n.is_empty());
            (is_terminal, has_children)
        };
        let remove_result = if has_children {
            Err(RemoveError::NonEmpty(path_elem))
        } else {
            let child_sequence = child.sequence();
            if child_sequence == sequence.sequence() {
                Ok(self
                    .nodes
                    .ref_mut()
                    .remove(path_elem)
                    .map(|(weight, node)| {
                        let (child_weights, info_intrinsic) = NodeInfo::from(node).into();
                        if !is_terminal {
                            assert!(child_weights.is_empty());
                        }
                        (weight, info_intrinsic)
                    })
                    .expect("node at index exists just after getting some"))
            } else {
                Err(RemoveError::SequenceMismatch(path_elem, child_sequence))
            }
        };
        Ok(remove_result)
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
        weight_vec::{OrderVec, Weights},
        SequenceAndItem,
    };

    use super::Node;
    /// Serializable representation of a filter/queue/merge element in the [`Tree`](`crate::Tree`)
    #[must_use]
    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub(crate) struct NodeInfo<T, F>(Weights, NodeInfoIntrinsic<T, F>);
    impl<T, F> From<NodeInfo<T, F>> for (Weights, NodeInfoIntrinsic<T, F>) {
        fn from(node_info: NodeInfo<T, F>) -> Self {
            (node_info.0, node_info.1)
        }
    }
    impl<T, F> From<Node<T, F>> for NodeInfo<T, F> {
        fn from(node: Node<T, F>) -> Self {
            let Node {
                children,
                queue,
                queue_prefill_len,
                filter,
                sequence: _,
            } = node;
            let (mut weights, info) = match children {
                Children::Chain(Chain { nodes }) => {
                    let (order, (weights, _nodes)) = nodes.into_parts();
                    let info = NodeInfoIntrinsic::Chain {
                        queue,
                        queue_prefill_len,
                        filter,
                        order,
                    };
                    (weights, info)
                }
                Children::Items(items) => {
                    let (order, (weights, items)) = items.into_parts();
                    let info = NodeInfoIntrinsic::Items {
                        queue,
                        queue_prefill_len,
                        items,
                        filter,
                        order,
                    };
                    (weights, info)
                }
            };
            weights.try_simplify();
            Self(weights, info)
        }
    }
    #[allow(clippy::trivially_copy_pass_by_ref)] // required by serde derive
    fn skip_if_zero(queue_prefill_len: &usize) -> bool {
        *queue_prefill_len == 0
    }
    /// Intrinsic description of a Node
    #[must_use]
    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(untagged)]
    pub enum NodeInfoIntrinsic<T, F> {
        /// Node containing only child items (no child nodes)
        // NOTE: need to put the "larger" variant first, otherwise Serde-untagged falls over
        // (picks first matching variant, apparently)
        Items {
            /// Items
            items: Vec<T>,
            /// Items manually queued, or prefilled from next available
            queue: VecDeque<SequenceAndItem<T>>,
            /// Minimum number of items to retain in queue (best-effort)
            #[serde(default, rename = "prefill", skip_serializing_if = "skip_if_zero")]
            queue_prefill_len: usize,
            /// Filtering value
            filter: F,
            /// Ordering type for child items
            order: order::Type,
        },
        /// Node containing nodes as children
        Chain {
            /// Items manually queued, or prefilled from next available
            queue: VecDeque<SequenceAndItem<T>>,
            /// Minimum number of items to retain in queue (best-effort)
            #[serde(default, rename = "prefill", skip_serializing_if = "skip_if_zero")]
            queue_prefill_len: usize,
            /// Filtering value
            filter: F,
            /// Ordering type for child nodes
            order: order::Type,
        },
    }
    impl<T, F: Default> Default for NodeInfoIntrinsic<T, F> {
        fn default() -> Self {
            Self::default_with_filter(F::default())
        }
    }
    impl<T, F> NodeInfoIntrinsic<T, F> {
        pub(crate) fn default_with_filter(filter: F) -> Self {
            Self::Chain {
                queue: VecDeque::default(),
                queue_prefill_len: 0,
                filter,
                order: order::Type::default(),
            }
        }
        pub(crate) fn construct_root(self) -> (Node<T, F>, SequenceCounter) {
            let root = self.make_node(id::ROOT.sequence());
            let counter = SequenceCounter::new(id::ROOT);
            (root, counter)
        }
        pub(crate) fn construct(self, counter: &mut SequenceCounter) -> Node<T, F> {
            self.make_node(counter.next())
        }
        fn make_node(self, sequence: Sequence) -> Node<T, F> {
            match self {
                Self::Chain {
                    queue,
                    queue_prefill_len,
                    filter,
                    order,
                } => Node {
                    children: Chain::new(order).into(),
                    queue,
                    queue_prefill_len,
                    filter,
                    sequence,
                },
                Self::Items {
                    items,
                    queue,
                    queue_prefill_len,
                    filter,
                    order,
                } => Node {
                    children: OrderVec::from((order, items.into_iter().map(|item| (1, item))))
                        .into(),
                    queue,
                    queue_prefill_len,
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
        fn new(from_id: NodeId<ty::Root>) -> Self {
            Self(from_id.sequence())
        }
        /// Returns the next Sequence value in the counter
        fn next(&mut self) -> Sequence {
            self.0 += 1;
            self.0
        }
    }
}
