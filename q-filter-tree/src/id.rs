// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Paths and Identifiers for nodes
use std::collections::VecDeque;

/// Representation for Root ID
pub(crate) const ROOT: NodeId<Root> = NodeId {
    path: NodePath::empty(),
    sequence: 0,
};

/// Element of a [`NodePath`]
pub type NodePathElem = usize;

pub use sequence::Sequence;
pub(crate) use sequence::{Keeper, SequenceSource};
mod sequence {
    use super::{ty, NodeId};

    /// Type of [`NodeId.sequence()`](`super::NodeId.sequence()`) for keeping unique identifiers for nodes
    pub type Sequence = u64;

    pub(crate) trait SequenceSource {
        fn sequence(&self) -> Sequence;
        fn sequence_keeper(&self) -> Keeper {
            Keeper(self.sequence())
        }
    }
    impl<T: ty::Type> SequenceSource for NodeId<T> {
        fn sequence(&self) -> Sequence {
            self.sequence()
        }
    }

    /// Opaque wrapper of a Sequence (to allow storing, and re-use)
    pub struct Keeper(Sequence);
    impl Keeper {
        /// Converts a number to a "trusted" Sequence (for use with user input)
        // Long name is itentional, to discourage use where a trusted [`SequenceSource`] is available
        pub(crate) fn assert_valid_sequence_from_user(num: Sequence) -> Self {
            Self(num)
        }
    }
    impl SequenceSource for Keeper {
        fn sequence(&self) -> Sequence {
            self.0
        }
    }
}

use ty::{Child, Root, Type};
/// Type Parameters for [`NodeId`] or [`NodePath`]
pub mod ty {
    use super::NodePathElem;

    /// Type Parameter for a [`NodeId`](`super::NodeId`) or [`NodePath`](`super::NodePath`)
    pub trait Type: private::Sealed + Clone {
        /// Returns a slice of the [`NodePathElem`]s
        fn elems(&self) -> &[NodePathElem];
        /// Moves elements out of self
        fn into_elems(self) -> Vec<NodePathElem>;
    }

    /// The referrent node has a parent
    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    pub struct Child(Vec<NodePathElem>);

    /// The referrent node has no parent (e.g. root)
    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    pub struct Root;

    mod private {
        pub trait Sealed {}
        impl Sealed for super::Child {}
        impl Sealed for super::Root {}
    }
    impl Child {
        /// Constructs a new [`Child`], if the supplied elements Vec is nonempty
        #[must_use]
        pub fn new(elems: Vec<NodePathElem>) -> Option<Self> {
            if elems.is_empty() {
                None
            } else {
                Some(Self(elems))
            }
        }
    }
    impl Type for Child {
        fn elems(&self) -> &[NodePathElem] {
            &self.0
        }
        fn into_elems(self) -> Vec<NodePathElem> {
            self.0
        }
    }
    impl Type for Root {
        fn elems(&self) -> &[NodePathElem] {
            &[]
        }
        fn into_elems(self) -> Vec<NodePathElem> {
            vec![]
        }
    }
}

shared::wrapper_enum! {
    /// Typed [`NodeId`]
    #[must_use]
    #[derive(Clone, PartialEq, Eq)]
    pub enum NodeIdTyped {
        /// Root id
        Root(NodeId<Root>),
        /// Child id
        Child(NodeId<Child>),
    }
    /// Typed [`NodePath`]
    #[must_use]
    #[derive(Clone, PartialEq, Eq, Hash)]
    pub enum NodePathTyped {
        /// Root path
        Root(NodePath<Root>),
        /// Child path
        Child(NodePath<Child>),
    }
}
shared::wrapper_enum! {
    /// Typed reference to a [`NodePath`]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum NodePathRefTyped<'a> {
        /// Root path
        Root(&'a NodePath<Root>),
        /// Child path
        Child(&'a NodePath<Child>),
    }
//  this seems like a PAIN to implement... for no real gain / use.
//  Just carry a (NodePathRefTyped, Sequence) !
//
//     /// Typed reference to a [`NodeId`]
//     #[derive(Debug, Clone, Copy, PartialEq, Eq)]
//     pub enum NodeIdRefTyped<'a> {
//         /// Root id
//         Root(ref NodeId<Root>),
//         /// Child id
//         Child(ref NodeId<Child>),
//     }
}

impl NodePathTyped {
    pub(crate) fn with_sequence<S: SequenceSource>(self, source: &S) -> NodeIdTyped {
        match self {
            Self::Root(path) => path.with_sequence(source).into(),
            Self::Child(path) => path.with_sequence(source).into(),
        }
    }
    pub(crate) fn append(self, next: NodePathElem) -> NodePath<Child> {
        match self {
            Self::Root(root) => root.append(next),
            Self::Child(child) => child.append(next),
        }
    }
    pub(crate) fn elems(&self) -> &[NodePathElem] {
        match self {
            Self::Root(path) => path.elems(),
            Self::Child(path) => path.elems(),
        }
    }
}
impl<'a> NodePathRefTyped<'a> {
    pub(crate) fn clone_inner(&self) -> NodePathTyped {
        match self {
            Self::Root(path) => NodePathTyped::Root((*path).clone()),
            Self::Child(path) => NodePathTyped::Child((*path).clone()),
        }
    }
}
impl<'a> std::ops::Deref for NodePathRefTyped<'a> {
    type Target = [NodePathElem];

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Root(path) => path.elems(),
            Self::Child(path) => path.elems(),
        }
    }
}
impl SequenceSource for NodeIdTyped {
    fn sequence(&self) -> Sequence {
        match self {
            Self::Root(node_id) => node_id.sequence(),
            Self::Child(node_id) => node_id.sequence(),
        }
    }
}

/// Unique identifier for a node in the [`Tree`](`super::Tree`)
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, PartialEq, Eq)]
pub struct NodeId<T: Type> {
    path: NodePath<T>,
    sequence: Sequence,
}

/// Path to a node in the [`Tree`](`crate::Tree`)
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct NodePath<T: Type>(T);

impl NodePath<Root> {
    pub(super) const fn empty() -> Self {
        Self(Root)
    }
}
impl NodePath<Child> {
    pub(super) fn new(elems: Vec<NodePathElem>) -> Option<Self> {
        Some(Self(Child::new(elems)?))
    }
}
impl<T: Type> NodePath<T> {
    /// Returns a slice of the [`NodePathElem`]s
    #[must_use]
    pub fn elems(&self) -> &[NodePathElem] {
        self.0.elems()
    }
    /// Moves elements out of self
    pub fn into_elems(self) -> Vec<NodePathElem> {
        self.0.into_elems()
    }
}

impl<T: Type> NodePath<T> {
    /// Appends a path element
    #[must_use]
    pub(crate) fn append(self, next: NodePathElem) -> NodePath<Child> {
        let mut parts = self.into_elems();
        parts.push(next);
        NodePath::new(parts).expect("appended part makes Vec nonempty")
    }
    pub(crate) fn with_sequence<S: SequenceSource>(self, source: &S) -> NodeId<T> {
        let sequence = source.sequence();
        self.with_sequence_unchecked(sequence)
    }
    fn with_sequence_unchecked(self, sequence: Sequence) -> NodeId<T> {
        NodeId {
            path: self,
            sequence,
        }
    }
    /// Returns `true` if the path is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.elems().is_empty()
    }
}
impl NodePath<Child> {
    /// Returns the parent path sequence (if it exists) and the last path element
    pub fn into_parent(self) -> (NodePathTyped, NodePathElem) {
        let mut parts = self.into_elems();
        let last_elem = parts.pop().expect("NodePath<Child> is not empty");
        (NodePathTyped::from(parts), last_elem)
    }
}
impl<T: Type> NodeId<T> {
    /// Returns the sequence identifier for the node
    #[must_use]
    pub fn sequence(&self) -> Sequence {
        self.sequence
    }
    /// Returns the inner [`NodePath`]
    pub fn into_inner(self) -> NodePath<T> {
        self.path
    }
}
impl<T: Type> std::ops::Deref for NodeId<T> {
    type Target = NodePath<T>;
    fn deref(&self) -> &Self::Target {
        &self.path
    }
}

impl From<Vec<NodePathElem>> for NodePathTyped {
    fn from(elems: Vec<NodePathElem>) -> Self {
        NodePath::new(elems).map_or_else(|| NodePath::empty().into(), NodePathTyped::from)
    }
}
impl From<VecDeque<NodePathElem>> for NodePathTyped {
    fn from(elems: VecDeque<NodePathElem>) -> Self {
        elems.into_iter().collect::<Vec<_>>().into()
    }
}
impl<'a, T: Type> From<&'a NodeId<T>> for &'a [NodePathElem] {
    fn from(node_id: &'a NodeId<T>) -> &'a [NodePathElem] {
        (&node_id.path).into()
    }
}
impl From<NodeIdTyped> for NodePathTyped {
    fn from(node_id: NodeIdTyped) -> Self {
        match node_id {
            NodeIdTyped::Root(node_id) => NodePath::from(node_id).into(),
            NodeIdTyped::Child(node_id) => NodePath::from(node_id).into(),
        }
    }
}
impl<'a> From<&'a NodeIdTyped> for NodePathRefTyped<'a> {
    fn from(node_id: &'a NodeIdTyped) -> Self {
        match node_id {
            NodeIdTyped::Root(node_id) => (&**node_id).into(),
            NodeIdTyped::Child(node_id) => (&**node_id).into(),
        }
    }
}
impl<'a> From<&'a NodePathTyped> for NodePathRefTyped<'a> {
    fn from(node_path: &'a NodePathTyped) -> Self {
        match node_path {
            NodePathTyped::Root(node_path) => node_path.into(),
            NodePathTyped::Child(node_path) => node_path.into(),
        }
    }
}
impl<'a> From<&'a NodePathTyped> for &'a [NodePathElem] {
    fn from(node_path: &'a NodePathTyped) -> Self {
        match node_path {
            NodePathTyped::Root(node_path) => node_path.into(),
            NodePathTyped::Child(node_path) => node_path.into(),
        }
    }
}
impl<'a, T: Type> From<&'a NodePath<T>> for &'a [NodePathElem] {
    fn from(node_path: &'a NodePath<T>) -> Self {
        node_path.elems()
    }
}
impl<T: Type> From<NodeId<T>> for NodePath<T> {
    fn from(node_id: NodeId<T>) -> Self {
        node_id.path
    }
}
impl<T: Type> From<NodeId<T>> for NodePathTyped
where
    NodePathTyped: From<NodePath<T>>,
{
    fn from(node_id: NodeId<T>) -> Self {
        NodePath::from(node_id).into()
    }
}

impl<T: Type> std::fmt::Debug for NodePath<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self.elems())
    }
}
impl<T: Type> std::fmt::Debug for NodeId<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}#{:?}", self.path, self.sequence)
    }
}
impl std::fmt::Debug for NodeIdTyped {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Root(id) => write!(f, "RootId({:?})", id),
            Self::Child(id) => write!(f, "ChildId({:?})", id),
        }
    }
}
impl std::fmt::Debug for NodePathTyped {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Root(path) => write!(f, "RootPath({:?})", path),
            Self::Child(path) => write!(f, "ChildPath({:?})", path),
        }
    }
}

// TODO: remove this, the need is not apparent
// #[derive(Default, Debug)]
// pub(crate) struct NodePathBuilder(VecDeque<NodePathElem>);
// impl NodePathBuilder {
//     pub fn prepend(&mut self, elem: NodePathElem) {
//         self.0.push_front(elem);
//     }
//     pub fn finish(self) -> NodePathTyped {
//         self.0.into()
//     }
// }
// #[derive(Debug)]
// pub(crate) struct NodeIdBuilder {
//     path: NodePathBuilder,
//     sequence: Sequence,
// }
// impl NodeIdBuilder {
//     pub fn new(sequence: Sequence) -> Self {
//         Self {
//             path: NodePathBuilder::default(),
//             sequence,
//         }
//     }
//     pub fn prepend(&mut self, elem: NodePathElem) {
//         self.path.prepend(elem);
//     }
//     pub fn finish(self) -> NodeIdTyped {
//         let Self { path, sequence } = self;
//         let path = path.finish();
//         match path {
//             NodePathTyped::Root(path) => path.with_sequence_unchecked(sequence).into(),
//             NodePathTyped::Child(path) => path.with_sequence_unchecked(sequence).into(),
//         }
//     }
// }
