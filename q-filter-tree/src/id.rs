//! Paths and Identifiers for nodes

/// Representation for Root ID
pub const ROOT: NodeId = NodeId(vec![]);

#[allow(clippy::module_name_repetitions)]
/// Element of a [`NodeId`]
pub type NodeIdElem = usize;

/// Identifier for a node in the [`Tree`](`super::Tree`)
#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeId(Vec<NodeIdElem>);
impl NodeId {
    /// Appends an element to the ID
    #[must_use]
    pub fn extend(&self, next: NodeIdElem) -> NodeId {
        let mut parts = self.0.clone();
        parts.push(next);
        Self(parts)
    }
    /// Returns the parent ID (if it exists) and last element
    #[must_use]
    pub fn parent(&self) -> Option<(NodeId, NodeIdElem)> {
        let mut parts = self.0.clone();
        parts.pop().map(|last_elem| (Self(parts), last_elem))
    }
    pub(crate) fn first_elem(&self) -> Option<NodeIdElem> {
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
