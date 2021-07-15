#![allow(clippy::module_name_repetitions)]
use crate::id::{NodeId, NodePath, NodePathElem, Sequence};

/// Error for an invalid [`NodeId`] path
#[derive(Debug, PartialEq, Eq)]
pub struct InvalidNodePath(NodePath);
impl From<&[NodePathElem]> for InvalidNodePath {
    fn from(node_id: &[NodePathElem]) -> Self {
        Self(node_id.to_vec().into())
    }
}

/// Error from item-pop operations
#[derive(Debug, PartialEq, Eq)]
pub enum PopError<T> {
    /// Terminal node has an empty queue (needs push)
    Empty(T),
    /// Child nodes are not allowed (all weights = 0)
    Blocked(T),
}
impl<T> PopError<T> {
    pub(crate) fn map_inner<U, F: Fn(T) -> U>(self, f: F) -> PopError<U> {
        match self {
            Self::Empty(inner) => PopError::Empty(f(inner)),
            Self::Blocked(inner) => PopError::Blocked(f(inner)),
        }
    }
}

/// Error from node-remove operations
/// Generic parameters set to:
/// - `T`=[`NodePath`] is the path for parent node
/// - `U`=[`NodePath`] is the path for children of the parent node
/// - `V`=[`NodeId`] is the id for the target removal node
///
pub type RemoveError = RemoveErrorGeneric<NodePath, NodePath, NodeId>;
pub(crate) type RemoveErrorInner = RemoveErrorGeneric<(), NodePathElem, ()>;
/// Error from the node-remove operations
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
    pub(crate) fn attach_id(self, node_id: &NodeId) -> RemoveError {
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
