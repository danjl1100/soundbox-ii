use crate::{
    error::{InvalidNodePath, PopError, RemoveErrorInner},
    id::{self, ty, NodeId, NodePathElem, Sequence, SequenceSource},
    order::{self, weight_vec, WeightVec},
    Weight,
};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Serializable representation of a filter/queue/merge element in the [`Tree`](`crate::Tree`)
#[must_use]
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct NodeInfo<T, F> {
    /// Items queue
    queue: VecDeque<T>,
    /// Filtering value
    filter: Option<F>,
    // TODO
    // /// Minimum number of items to retain in queue, beyond which [`PopError::NeedsPush`] is raised
    // pub retain_count: usize,
    child_weights: Vec<Weight>,
    order: order::Type,
}
/// Intrinsic fields of [`NodeInfo`]
#[must_use]
pub(crate) struct NodeInfoIntrinsic<T, F> {
    /// Items queue
    queue: VecDeque<T>,
    /// Filtering value
    filter: Option<F>,
    // TODO
    // /// Minimum number of items to retain in queue, beyond which [`PopError::NeedsPush`] is raised
    // pub retain_count: usize,
    order: order::Type,
}
impl<'a, T, F> From<NodeInfo<T, F>> for (NodeInfoIntrinsic<T, F>, Vec<Weight>) {
    fn from(other: NodeInfo<T, F>) -> Self {
        let NodeInfo {
            queue,
            filter,
            child_weights,
            order,
        } = other;
        let intrinsic = NodeInfoIntrinsic {
            queue,
            filter,
            order,
        };
        (intrinsic, child_weights)
    }
}
impl<'a, T: Clone, F: Clone> From<&'a Node<T, F>> for NodeInfo<T, F> {
    fn from(node: &'a Node<T, F>) -> Self {
        let Node {
            queue,
            filter,
            children,
            order,
            ..
        } = node;
        Self {
            queue: queue.clone(),
            filter: filter.clone(),
            child_weights: children.weights().into(),
            order: order.into(),
        }
    }
}

/// Internal representation of a filter/queue/merge element in the [`Tree`](`crate::Tree`)
#[must_use]
#[derive(PartialEq, Eq)]
pub struct Node<T, F> {
    /// Items queue
    pub queue: VecDeque<T>,
    /// Filtering value
    pub filter: Option<F>,
    // TODO
    // /// Minimum number of items to retain in queue, beyond which [`PopError::NeedsPush`] is raised
    // pub retain_count: usize,
    children: WeightVec<Node<T, F>>,
    order: order::State,
    sequence: Sequence,
}
impl<T, F> Node<T, F> {
    pub(crate) fn new(counter: &mut SequenceCounter) -> Self {
        Self::new_with_seq(counter.next())
    }
    pub(crate) fn new_root() -> (Self, SequenceCounter) {
        const ROOT_ID: NodeId<ty::Root> = id::ROOT;
        let root = Self::new_with_seq(ROOT_ID.sequence());
        let counter = SequenceCounter::new(&ROOT_ID);
        (root, counter)
    }
    fn new_with_seq(sequence: Sequence) -> Self {
        Self {
            queue: VecDeque::new(),
            filter: None,
            // TODO
            // retain_count: 0,
            children: WeightVec::new(),
            order: order::Type::InOrder.into(),
            sequence,
        }
    }
    /// Adds a child to the specified `Node`, with an optional `Weight`
    pub(crate) fn add_child(
        &mut self,
        weight: Option<Weight>,
        counter: &mut SequenceCounter,
    ) -> (NodePathElem, &Self) {
        let weight = weight.unwrap_or(0);
        let child_node = Node::new(counter);
        let child = {
            let child_part = self.children.len() as NodePathElem;
            // push AFTER recording length ^
            self.children
                .ref_mut(&mut self.order)
                .push((weight, child_node));
            let child_node = self.children.elems().last().expect("pushed element exists");
            (child_part, child_node)
        };
        self.order.clear();
        child
    }
    /// Removes the specified child node
    ///
    /// # Errors
    /// Returns an error if the specified `NodeId` does not point to a valid node,
    ///  or if the node has existing children.
    ///
    pub(crate) fn remove_child<S: SequenceSource>(
        &mut self,
        id_elem: NodePathElem,
        sequence_source: &S,
    ) -> Result<(Weight, Node<T, F>), RemoveErrorInner> {
        if let Some((_, child)) = self.children.get(id_elem) {
            let child_sequence = child.sequence();
            if child_sequence == sequence_source.sequence() {
                let child_children = child.get_child_nodes();
                if child_children.is_empty() {
                    self.order.clear();
                    Ok(self
                        .children
                        .ref_mut(&mut self.order)
                        .remove(id_elem)
                        .expect("indexed child still present for removal"))
                } else {
                    let child_id_elems = (0..child_children.len()).collect();
                    Err(RemoveErrorInner::NonEmpty((), child_id_elems))
                }
            } else {
                Err(RemoveErrorInner::SequenceMismatch((), child_sequence))
            }
        } else {
            Err(RemoveErrorInner::Invalid(()))
        }
    }
    fn get_child_nodes(&self) -> &[Node<T, F>] {
        self.children.elems()
    }
    /// Returns the child `Node` at the specified ID elements path
    ///
    /// # Errors
    /// Returns an error if the specified `NodeId` does not point to a valid node
    ///
    pub(crate) fn get_child(
        &self,
        id_elems: &[NodePathElem],
    ) -> Result<&Node<T, F>, InvalidNodePath> {
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
    pub(crate) fn get_child_mut(
        &mut self,
        id_elems: &[NodePathElem],
    ) -> Result<&mut Node<T, F>, InvalidNodePath> {
        if id_elems.is_empty() {
            Ok(self)
        } else {
            self.get_child_entry_mut(id_elems).map(|(_, node)| node)
        }
    }
    #[allow(clippy::type_complexity)] //TODO make return type more... straightforward
    /// Returns the child `Node` and associated `Weight` of the specified ID elements path
    ///
    /// # Errors
    /// Returns an error if the specified `NodeId` does not point to a valid node
    ///
    pub(crate) fn get_child_and_weight_parent_order_mut(
        &mut self,
        id_elems: &[NodePathElem],
    ) -> Result<
        Result<weight_vec::RefMutElem<'_, '_, Node<T, F>>, &'_ mut Node<T, F>>,
        InvalidNodePath,
    > {
        if id_elems.is_empty() {
            Ok(Err(self))
        } else {
            self.get_child_entry_mut(id_elems).map(Ok)
        }
    }
    /// Returns the child `Node` and index (if any), after the specified index
    pub(crate) fn get_idx_and_child_after(
        &self,
        after_idx: Option<usize>,
    ) -> Option<(usize, &Self)> {
        let idx = after_idx.map_or(0, |i| i + 1);
        let child = self.children.elems().get(idx);
        child.map(|c| (idx, c))
    }
    pub(crate) fn sum_node_count(&self) -> usize {
        let child_sum: usize = self.children.elems().iter().map(Self::sum_node_count).sum();
        child_sum + 1
    }
    fn get_child_entry(
        &self,
        id_elems: &[NodePathElem],
    ) -> Result<(Weight, &Node<T, F>), InvalidNodePath> {
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
        id_elems: &[NodePathElem],
    ) -> Result<weight_vec::RefMutElem<'_, '_, Node<T, F>>, InvalidNodePath> {
        if let Some((&this_idx, remainder)) = id_elems.split_first() {
            let child_ref = self
                .children
                .ref_mut(&mut self.order)
                .into_elem_ref(this_idx)
                .or(Err(id_elems))?;
            if remainder.is_empty() {
                Ok(child_ref)
            } else {
                let child_node = child_ref.1;
                child_node.get_child_entry_mut(remainder)
            }
        } else {
            Err(id_elems.into())
        }
    }
    /// Sets the weight of the specified `Node`
    ///
    /// # Errors
    /// Returns an error if the specified `NodeId` does not point to a valid node
    ///
    pub fn set_weight(
        &mut self,
        node_id: &[NodePathElem],
        weight: Weight,
    ) -> Result<(), InvalidNodePath> {
        let (mut weight_ref, _) = self.get_child_entry_mut(node_id)?;
        weight_ref.set_weight(weight);
        Ok(())
    }
    /// Sets the [`OrderType`](`crate::order::Type`) of this node
    pub fn set_order(&mut self, ty: order::Type) {
        self.order.set_type(ty);
    }
    /// Attempts to pop the next item, pulling from child nodes as needed
    ///
    /// # Errors
    /// Returns an error if the pop operation fails
    ///
    pub fn pop_item(&mut self) -> Result<T, PopError<()>> {
        self.queue.pop_front().ok_or(()).or_else(|_| {
            if self.children.is_empty() {
                Err(PopError::Empty(()))
            } else {
                let weights = self.children.weights();
                let child_idx = self.order.next(weights);
                let child = if let Some(child_idx) = child_idx {
                    self.children.get_elem_mut(child_idx)
                } else {
                    None
                };
                if let Some(child) = child {
                    child.pop_item()
                } else {
                    Err(PopError::Blocked(()))
                }
            }
        })
    }
    pub(crate) fn overwrite_from(&mut self, info: NodeInfoIntrinsic<T, F>) {
        let NodeInfoIntrinsic {
            queue,
            filter,
            order,
        } = info;
        self.queue = queue;
        self.filter = filter;
        self.set_order(order);
    }
    pub(crate) fn overwrite_child_weights(
        &mut self,
        weights: Vec<Weight>,
    ) -> Result<(), (Vec<Weight>, usize)> {
        self.children
            .ref_mut(&mut self.order)
            .overwrite_weights(weights)
    }
}
impl<T, F> SequenceSource for Node<T, F> {
    fn sequence(&self) -> Sequence {
        self.sequence
    }
}
impl<T, F> std::fmt::Debug for Node<T, F>
where
    T: std::fmt::Debug,
    F: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "Node(#{}, q={:?}, f={:?}, {:?}, w={:?})",
            self.sequence,
            self.queue,
            self.filter,
            order::Type::from(&self.order),
            self.children.weights()
        )
        // f.debug_map()
        //     .entries(self.children.elems().iter().enumerate())
        //     .finish()
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
