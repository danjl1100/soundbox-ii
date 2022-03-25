//! Various error types associated with [`Tree`](`crate::Tree`) methods

#![allow(clippy::module_name_repetitions)]
use crate::id::{NodePathElem, NodePathTyped, Sequence};

/// Error for an invalid [`NodePathTyped`]
#[derive(Debug, PartialEq, Eq)]
pub struct InvalidNodePath(NodePathTyped);
impl<T> From<T> for InvalidNodePath
where
    T: Into<NodePathTyped>,
{
    fn from(node_path: T) -> Self {
        Self(node_path.into())
    }
}
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
// impl<T> PopError<T> {
//     pub(crate) fn map_inner<U, F: Fn(T) -> U>(self, f: F) -> PopError<U> {
//         match self {
//             Self::Empty(inner) => PopError::Empty(f(inner)),
//             Self::Blocked(inner) => PopError::Blocked(f(inner)),
//         }
//     }
// }

/// Error removing a node (when node is indeed found)
#[derive(Debug, PartialEq)]
pub enum RemoveError<T> {
    /// Node matching the path has a different sequence (e.g. node paths changed)
    SequenceMismatch(T, Sequence),
    /// Node has outstanding children
    NonEmpty(T),
}
impl<T> RemoveError<T> {
    pub(crate) fn map_id<U, F>(self, id_fn: F) -> RemoveError<U>
    where
        F: FnOnce(T) -> U,
    {
        match self {
            Self::SequenceMismatch(id, sequence) => {
                RemoveError::SequenceMismatch(id_fn(id), sequence)
            }
            Self::NonEmpty(id) => RemoveError::NonEmpty(id_fn(id)),
        }
    }
}
