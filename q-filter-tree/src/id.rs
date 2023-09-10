// Copyright (C) 2021-2023  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
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
pub use sequence::{Keeper, SequenceSource};
mod sequence {
    use super::{ty, NodeId};

    /// Type of [`NodeId.sequence()`](`super::NodeId.sequence()`) for keeping unique identifiers for nodes
    pub type Sequence = u64;

    mod private {
        pub trait Sealed {}
        impl Sealed for super::Keeper {}
        impl Sealed for super::super::NodeIdTyped {}
        impl Sealed for super::super::NodeIdRefTyped<'_> {}
        impl<T: super::ty::Type> Sealed for super::NodeId<T> {}
        impl<T, F> Sealed for crate::Node<T, F> {}
        impl<T, F> Sealed for crate::refs::NodeRefMut<'_, '_, T, F> {}
    }

    /// Source of an immutable identity / sequence number
    #[allow(clippy::module_name_repetitions)]
    pub trait SequenceSource: private::Sealed {
        /// Returns the item's sequence identifier
        ///
        /// NOTE: This is only valid for the current runtime instantiation (not for serialization)
        fn sequence(&self) -> Sequence;
        /// Returns a wrapper denoting this sequence came from an actual object (not raw user input)
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
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Root;

    mod private {
        pub trait Sealed {}
        impl Sealed for super::Child {}
        impl Sealed for super::Root {}
    }
    impl Child {
        /// Constructor ensures the inner `Vec` is non-empty
        const NONEMPTY: &str = "nonempty by construction";
        /// Constructs a new [`Child`], if the supplied elements Vec is nonempty
        #[must_use]
        pub fn new(elems: Vec<NodePathElem>) -> Option<Self> {
            if elems.is_empty() {
                None
            } else {
                Some(Self(elems))
            }
        }
        /// Returns a slice of the [`NodePathElem`]s, with `split_last` already applied
        #[allow(clippy::missing_panics_doc)] // guaranteed by type
        #[must_use]
        pub fn elems_split_last(&self) -> (NodePathElem, &[NodePathElem]) {
            let (last, elems) = self.elems().split_last().expect(Self::NONEMPTY);
            (*last, elems)
        }

        /// Splits into the remaining elements and the last element
        #[allow(clippy::missing_panics_doc)] // guaranteed by type
        #[must_use]
        pub fn into_split_last(self) -> (Vec<NodePathElem>, NodePathElem) {
            let Self(mut parts) = self;
            let last_elem = parts.pop().expect(Self::NONEMPTY);
            (parts, last_elem)
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

/// Typed [`NodeId`]
#[must_use]
#[derive(Clone, PartialEq, Eq)]
pub struct NodeIdTyped {
    path: NodePathTyped,
    sequence: Sequence,
}
impl<T: Type> From<NodeId<T>> for NodeIdTyped
where
    NodePathTyped: From<NodePath<T>>,
{
    fn from(node_id: NodeId<T>) -> Self {
        let NodeId { path, sequence } = node_id;
        Self {
            path: path.into(),
            sequence,
        }
    }
}

shared::wrapper_enum! {
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
}
/// Typed reference to a [`NodeId`]
#[derive(Clone, Copy)]
pub struct NodeIdRefTyped<'a> {
    path: NodePathRefTyped<'a>,
    sequence: Sequence,
}
impl SequenceSource for NodeIdRefTyped<'_> {
    fn sequence(&self) -> Sequence {
        self.sequence
    }
}

impl NodePathTyped {
    /// Creates a `NodeIdTyped` with the specified `Sequence`
    pub fn with_sequence<S: SequenceSource>(self, source: &S) -> NodeIdTyped {
        match self {
            Self::Root(path) => path.with_sequence(source).into(),
            Self::Child(path) => path.with_sequence(source).into(),
        }
    }
    /// Appends the specified element to the path
    pub fn append(self, next: NodePathElem) -> NodePath<Child> {
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
    pub(crate) fn as_ref(&self) -> NodePathRefTyped<'_> {
        match self {
            Self::Root(path) => path.into(),
            Self::Child(path) => path.into(),
        }
    }
}
impl<'a> NodePathRefTyped<'a> {
    pub(crate) fn clone_inner(&self) -> NodePathTyped {
        match self {
            Self::Root(path) => NodePathTyped::Root(**path),
            Self::Child(path) => NodePathTyped::Child((*path).clone()),
        }
    }
    pub(crate) fn elems(&self) -> &[NodePathElem] {
        match self {
            Self::Root(path) => path.elems(),
            Self::Child(path) => path.elems(),
        }
    }
}
impl<'a> PartialEq<NodePathTyped> for NodePathRefTyped<'a> {
    fn eq(&self, other: &NodePathTyped) -> bool {
        match (self, other) {
            (Self::Root(l0), NodePathTyped::Root(r0)) => *l0 == r0,
            (Self::Child(l0), NodePathTyped::Child(r0)) => *l0 == r0,
            (Self::Root(..), NodePathTyped::Child(..))
            | (Self::Child(..), NodePathTyped::Root(..)) => false,
        }
    }
}
impl SequenceSource for NodeIdTyped {
    fn sequence(&self) -> Sequence {
        self.sequence
    }
}

/// Unique identifier for a node in the [`Tree`](`super::Tree`)
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct NodeId<T: Type> {
    path: NodePath<T>,
    sequence: Sequence,
}

/// Path to a node in the [`Tree`](`crate::Tree`)
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
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
    pub(super) fn new_push(mut elems: Vec<NodePathElem>, push_elem: NodePathElem) -> Self {
        elems.push(push_elem);
        Self::new(elems).expect("Vec nonempty after push")
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
impl NodePath<ty::Child> {
    /// Returns a slice of the [`NodePathElem`]s, with `split_last` already applied
    #[must_use]
    pub fn elems_split_last(&self) -> (NodePathElem, &[NodePathElem]) {
        self.0.elems_split_last()
    }
}

impl<T: Type> NodePath<T> {
    /// Appends a path element
    pub fn append(self, next: NodePathElem) -> NodePath<Child> {
        let parts = self.into_elems();
        NodePath::new_push(parts, next)
    }
    /// Creates a `NodeId` with the specified `Sequence`
    pub fn with_sequence<S: SequenceSource>(self, source: &S) -> NodeId<T> {
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
        let (parts, last_elem) = self.0.into_split_last();
        (parts.into(), last_elem)
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
        Self::from_iter(elems)
    }
}
impl FromIterator<NodePathElem> for NodePathTyped {
    fn from_iter<T: IntoIterator<Item = NodePathElem>>(iter: T) -> Self {
        iter.into_iter().collect::<Vec<_>>().into()
    }
}
impl<'a, T: Type> From<&'a NodeId<T>> for &'a [NodePathElem] {
    fn from(node_id: &'a NodeId<T>) -> &'a [NodePathElem] {
        (&node_id.path).into()
    }
}
impl From<NodeIdTyped> for NodePathTyped {
    fn from(node_id: NodeIdTyped) -> Self {
        node_id.path
    }
}
impl<'a> From<&'a NodeId<ty::Root>> for NodePathRefTyped<'a> {
    fn from(node_id: &'a NodeId<ty::Root>) -> Self {
        Self::Root(node_id)
    }
}
impl<'a> From<&'a NodeId<ty::Child>> for NodePathRefTyped<'a> {
    fn from(node_id: &'a NodeId<ty::Child>) -> Self {
        Self::Child(node_id)
    }
}
impl<'a> From<&'a NodeIdTyped> for NodePathRefTyped<'a> {
    fn from(node_id: &'a NodeIdTyped) -> Self {
        (&node_id.path).into()
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
impl<'a> From<&'a NodeIdTyped> for NodeIdRefTyped<'a> {
    fn from(node_id: &'a NodeIdTyped) -> Self {
        let NodeIdTyped { ref path, sequence } = *node_id;
        Self {
            path: path.into(),
            sequence,
        }
    }
}
impl<'a> From<NodeIdRefTyped<'a>> for NodePathRefTyped<'a> {
    fn from(node_id: NodeIdRefTyped<'a>) -> Self {
        node_id.path
    }
}
impl<'a, T: Type> From<&'a NodeId<T>> for NodeIdRefTyped<'a>
where
    NodePathRefTyped<'a>: From<&'a NodeId<T>>,
{
    fn from(node_id: &'a NodeId<T>) -> Self {
        let sequence = node_id.sequence();
        NodeIdRefTyped {
            path: node_id.into(),
            sequence,
        }
    }
}

impl NodePathRefTyped<'_> {
    /// Clones the inner path, to construct [`NodePathTyped`]
    /// (possibly expensive, so not appropriate for [`From`])
    pub fn clone_owned(&self) -> NodePathTyped {
        match *self {
            NodePathRefTyped::Root(node_path) => NodePathTyped::Root(*node_path),
            NodePathRefTyped::Child(node_path) => NodePathTyped::Child(node_path.clone()),
        }
    }
}

impl<T: Type> std::fmt::Debug for NodePath<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self.elems())
    }
}
impl<T: Type> std::fmt::Debug for NodeId<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let Self { path, sequence } = self;
        write!(f, "{path:?}#{sequence:?}")
    }
}
impl std::fmt::Debug for NodeIdTyped {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let Self { path, sequence } = self;
        f.debug_struct("NodeIdTyped")
            .field("path", path)
            .field("sequence", sequence)
            .finish()
    }
}
impl std::fmt::Debug for NodePathTyped {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Root(path) => write!(f, "RootPath({path:?})"),
            Self::Child(path) => write!(f, "ChildPath({path:?})"),
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
