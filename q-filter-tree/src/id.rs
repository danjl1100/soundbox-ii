//! Paths and Identifiers for nodes
use std::collections::VecDeque;

/// Representation for Root ID
pub(crate) const ROOT: NodeId = NodeId {
    path: NodePath(vec![]),
    sequence: 0,
};

/// Element of a [`NodePath`]
pub(crate) type NodePathElem = usize;

/// Type of [`NodeId.sequence()`] for keeping unique identifiers for nodes
pub(crate) type Sequence = u64;

/// Path to a node in the [`Tree`](`super::Tree`)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodePath(Vec<NodePathElem>);

/// Unique identifier for a node in the [`Tree`](`super::Tree`)
#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeId {
    path: NodePath,
    sequence: Sequence,
}

impl NodePath {
    /// Appends a path element
    #[must_use]
    pub(crate) fn extend(&self, next: NodePathElem) -> NodePath {
        let mut parts = self.0.clone();
        parts.push(next);
        Self(parts)
    }
    /// Returns the parent path sequence (if it exists) and the last path element
    #[must_use]
    pub fn parent(&self) -> Option<(NodePath, NodePathElem)> {
        let mut parts = self.0.clone();
        parts.pop().map(|last_elem| (Self(parts), last_elem))
    }
    pub(crate) fn first_elem(&self) -> Option<NodePathElem> {
        self.0.get(0).copied()
    }
    pub(crate) fn with_sequence(self, sequence: Sequence) -> NodeId {
        NodeId {
            path: self,
            sequence,
        }
    }
    /// Returns `true` if the path is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
impl NodeId {
    /// Returns the sequence identifier for the node
    #[must_use]
    pub fn sequence(&self) -> Sequence {
        self.sequence
    }
}
impl std::ops::Deref for NodeId {
    type Target = NodePath;
    fn deref(&self) -> &Self::Target {
        &self.path
    }
}

impl From<Vec<NodePathElem>> for NodePath {
    fn from(elems: Vec<NodePathElem>) -> Self {
        Self(elems)
    }
}
impl From<VecDeque<NodePathElem>> for NodePath {
    fn from(elems: VecDeque<NodePathElem>) -> Self {
        Self(elems.into_iter().collect())
    }
}
impl<'a> From<&'a NodeId> for &'a [NodePathElem] {
    fn from(node_id: &'a NodeId) -> &'a [NodePathElem] {
        (&node_id.path).into()
    }
}
impl<'a> From<&'a NodePath> for &'a [NodePathElem] {
    fn from(node_path: &'a NodePath) -> Self {
        node_path.0.as_slice()
    }
}
impl From<NodeId> for NodePath {
    fn from(node_id: NodeId) -> Self {
        node_id.path
    }
}

#[derive(Default, Debug)]
pub(crate) struct NodePathBuilder(VecDeque<NodePathElem>);
impl NodePathBuilder {
    pub fn prepend(mut self, elem: NodePathElem) -> Self {
        self.0.push_front(elem);
        self
    }
    pub fn finish(self) -> NodePath {
        self.0.into()
    }
}
#[derive(Debug)]
pub(crate) struct NodeIdBuilder {
    path: NodePathBuilder,
    sequence: Sequence,
}
impl NodeIdBuilder {
    pub fn new(sequence: Sequence) -> Self {
        Self {
            path: NodePathBuilder::default(),
            sequence,
        }
    }
    pub fn prepend(mut self, elem: NodePathElem) -> Self {
        self.path = self.path.prepend(elem);
        self
    }
    pub fn finish(self) -> NodeId {
        let Self { path, sequence } = self;
        NodeId {
            path: path.finish(),
            sequence,
        }
    }
}
