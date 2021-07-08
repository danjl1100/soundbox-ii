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

use std::collections::VecDeque;

type NodeIdElem = usize;

/// Identifier for a Node in the [`Tree`]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeId(Vec<NodeIdElem>);
const ROOT_ID: NodeId = NodeId(vec![]);
impl NodeId {
    fn extend(&self, next: NodeIdElem) -> NodeId {
        let mut parts = self.0.clone();
        parts.push(next);
        Self(parts)
    }
}

/// Numeric type for weighting nodes in the [`Tree`]
pub type Weight = u32;

/// Error for an invalid [`NodeId`]
#[derive(Debug)]
pub struct InvalidNodeId(NodeId);
impl From<&NodeId> for InvalidNodeId {
    fn from(node_id: &NodeId) -> Self {
        Self::from(node_id.0.as_slice())
    }
}
impl From<&[NodeIdElem]> for InvalidNodeId {
    fn from(node_id: &[NodeIdElem]) -> Self {
        Self(NodeId(node_id.to_vec()))
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
        ROOT_ID
    }
    fn get_node(&self, node_id: &NodeId) -> Result<&Node<T, F>, InvalidNodeId> {
        if node_id.0.is_empty() {
            Ok(&self.root)
        } else {
            self.root.get_node(&node_id.0)
        }
    }
    fn get_node_mut(&mut self, node_id: &NodeId) -> Result<&mut Node<T, F>, InvalidNodeId> {
        if node_id.0.is_empty() {
            Ok(&mut self.root)
        } else {
            self.root.get_node_mut(&node_id.0)
        }
    }
    pub fn add_child(
        &mut self,
        node_id: &NodeId,
        weight: Option<Weight>,
    ) -> Result<NodeId, InvalidNodeId> {
        let parent = self.get_node_mut(node_id)?;
        let child_part = parent.children.len() as NodeIdElem;
        let child_id = node_id.extend(child_part);
        let weight = weight.unwrap_or(0);
        parent.children.push((weight, Node::default()));
        Ok(child_id)
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
    pub fn pop_item_from(&mut self, node_id: &NodeId) -> Result<Option<T>, InvalidNodeId> {
        let node = self.get_node_mut(node_id)?;
        Ok(node.queue.pop_front())
    }
}

#[must_use]
#[derive(Debug, PartialEq, Eq)]
struct Node<T, F> {
    queue: VecDeque<T>,
    filter: F,
    children: Vec<(Weight, Node<T, F>)>,
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
        }
    }
}
impl<T, F> Node<T, F> {
    fn get_node(&self, node_id: &[NodeIdElem]) -> Result<&Node<T, F>, InvalidNodeId> {
        if let Some((&this_idx, remainder)) = node_id.split_first() {
            let weight_child = self.children.get(this_idx).ok_or(node_id)?;
            let (_, child) = weight_child;
            if remainder.is_empty() {
                Ok(child)
            } else {
                child.get_node(remainder)
            }
        } else {
            Err(node_id.into())
        }
    }
    fn get_node_mut(&mut self, node_id: &[NodeIdElem]) -> Result<&mut Node<T, F>, InvalidNodeId> {
        if let Some((&this_idx, remainder)) = node_id.split_first() {
            let weight_child = self.children.get_mut(this_idx).ok_or(node_id)?;
            let (_, child) = weight_child;
            if remainder.is_empty() {
                Ok(child)
            } else {
                child.get_node_mut(remainder)
            }
        } else {
            Err(node_id.into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Node, Tree};
    #[test]
    fn creates_single() {
        let mut t = Tree::new();
        let root_id = t.root_id();
        // item
        const N: usize = 10;
        for i in 0..N {
            t.push_item(&root_id, i).expect("root-id exists");
        }
        for i in 0..N {
            assert_eq!(t.pop_item_from(&root_id).expect("root-id exists"), Some(i));
        }
        assert_eq!(t.pop_item_from(&root_id).expect("root-id exists"), None);
        // filter
        t.set_filter(&root_id, String::from("my root"))
            .expect("root-id exists");
        assert_eq!(
            t.get_filter(&root_id).expect("root-id exists"),
            &String::from("my root")
        );
    }
    #[test]
    fn two_nodes() {
        let mut t = Tree::new();
        let root_id = t.root_id();
        //
        let child_id = t.add_child(&root_id, None).expect("root-id exists");
        // filter
        t.set_filter(&child_id, String::from("child_filter"))
            .expect("child-id exists");
        t.set_filter(&root_id, String::from("root_filter"))
            .expect("root-id exists");
        // item
        const N: usize = 5;
        for i in 0..N {
            t.push_item(&child_id, i).expect("child-id exists");
            t.push_item(&root_id, i + 500).expect("root-id exists");
        }
        for i in 0..N {
            assert_eq!(
                t.pop_item_from(&child_id).expect("child-id exists"),
                Some(i)
            );
            assert_eq!(
                t.pop_item_from(&root_id).expect("root-id exists"),
                Some(i + 500)
            );
        }
        assert_eq!(t.pop_item_from(&child_id).expect("child-id exists"), None);
        assert_eq!(t.pop_item_from(&root_id).expect("root-id exists"), None);
    }
}
