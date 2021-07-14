use super::{order, InvalidNodeId, NodeId, NodeIdElem, PopError, Weight};
use std::collections::VecDeque;

#[derive(Debug, PartialEq, Eq)]
struct WeightNodeVec<T, F>(Vec<Weight>, Vec<Node<T, F>>)
where
    F: Default;
impl<T, F> WeightNodeVec<T, F>
where
    F: Default,
{
    fn new() -> Self {
        Self(vec![], vec![])
    }
    fn len(&self) -> usize {
        self.0.len()
    }
    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    fn get(&self, index: usize) -> Option<(Weight, &Node<T, F>)> {
        match (self.0.get(index), self.1.get(index)) {
            (Some(&weight), Some(node)) => Some((weight, node)),
            _ => None,
        }
    }
    fn get_mut(&mut self, index: usize) -> Option<(&mut Weight, &mut Node<T, F>)> {
        match (self.0.get_mut(index), self.1.get_mut(index)) {
            (Some(weight), Some(node)) => Some((weight, node)),
            _ => None,
        }
    }
    fn weights(&self) -> &[Weight] {
        &self.0
    }
    fn nodes(&self) -> &[Node<T, F>] {
        &self.1
    }
    fn push(&mut self, (weight, node): (Weight, Node<T, F>)) {
        self.0.push(weight);
        self.1.push(node);
    }
    fn remove(&mut self, index: usize) -> (Weight, Node<T, F>) {
        (self.0.remove(index), self.1.remove(index))
    }
}

/// Internal representation of a filter/queue/merge element in the [`Tree`]
#[must_use]
#[derive(Debug, PartialEq, Eq)]
pub struct Node<T, F>
where
    F: Default,
{
    /// Items queue
    pub queue: VecDeque<T>,
    /// Filtering value
    pub filter: F,
    children: WeightNodeVec<T, F>,
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
            children: WeightNodeVec::new(),
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
        self.order.clear();
        // return new NodeId
        node_id.extend(child_part)
    }
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
    ) -> Result<(Weight, &Node<T, F>), InvalidNodeId> {
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
    ) -> Result<(&mut Weight, &mut Node<T, F>), InvalidNodeId> {
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
        self.order.clear();
        Ok(())
    }
    /// Attempts to pop the next item
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
                let child = self
                    .order
                    .next(weights)
                    .and_then(|index| self.children.get_mut(index));
                if let Some((_, child)) = child {
                    child.pop_item()
                } else {
                    Err(PopError::Blocked(()))
                }
            }
        })
    }
}
