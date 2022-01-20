//! [`Tree`] structure, where each node has a queue of items and a filter.

// TODO: only while building
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
#![deny(rustdoc::broken_intra_doc_links)]

use error::{InvalidNodePath, PopError, RemoveError};
pub mod error;

use id::{ty, NodeId, NodePath, NodePathElem};
pub mod id;

pub use node::Node;
mod node;

mod weight_vec;

pub use order::Type as OrderType;
pub mod order;

mod iter;
mod serde {
    mod node_path;
    mod tree;
}

/// Numeric type for weighting nodes in the [`Tree`], used by to fuel [`OrderType`] algorithms
pub type Weight = u32;

#[test]
#[ignore] // TODO
fn tree_add_to_doc_tests() {
    let mut tree: Tree<_, _> = Tree::new();
    let root = tree.root_id();
    //
    // TODO: add this compile-error to Doc Tests as failed test
    // compile error: assert!(tree.get_child_mut(&root).is_err());
    let mut root_ref = root.try_ref(&mut tree).expect("root exists");
    *root_ref.filter() = Some("filter value".to_string());
    let child_blocked = root_ref.add_child(None);
    let child = root_ref.add_child(Some(1));
    // initial weight `None` (0)
    child_blocked
        .try_ref(&mut tree)
        .expect("root exists")
        .push_item("apple");
    // initial weight `1`
    child
        .try_ref(&mut tree)
        .expect("child exists")
        .push_item("banana");
    //
    let mut root_ref = root.try_ref(&mut tree).expect("root exists");
    assert_eq!(root_ref.pop_item(), Ok("banana"));
    assert_eq!(
        root_ref.pop_item(),
        Err(PopError::Empty(root.clone().into()))
    );
    // unblock "child_blocked"
    child_blocked
        .try_ref(&mut tree)
        .expect("child_blocked exists")
        .set_weight(2);
    let child_unblocked = child_blocked;
    child_unblocked
        .try_ref(&mut tree)
        .expect("child_unblocked exists")
        .push_item("cashews");
    let mut root_ref = root.try_ref(&mut tree).expect("root exists");
    assert_eq!(root_ref.pop_item(), Ok("apple"));
    assert_eq!(root_ref.pop_item(), Ok("cashews"));
    assert_eq!(
        root_ref.pop_item(),
        Err(PopError::Empty(root.clone().into()))
    );
}
/// Tree data structure, consisting of [`Node`]s with queues of items `T`, filter `F`
///
/// # Example
/// ```
/// use q_filter_tree::{Tree, error::PopError};
/// let mut tree: Tree<_, _> = Tree::new();
/// let root = tree.root_id();
/// //
/// let mut root_ref = root.try_ref(&mut tree).expect("root exists");
/// *root_ref.filter() = Some("filter value".to_string());
/// let child_blocked = root_ref.add_child(None);
/// let child = root_ref.add_child(Some(1));
/// // initial weight `None` (0)
/// child_blocked.try_ref(&mut tree)
///     .expect("child_blocked exists")
///     .push_item("apple");
/// // initial weight `1`
/// child.try_ref(&mut tree)
///     .expect("child exists")
///     .push_item("banana");
/// //
/// let mut root_ref = root.try_ref(&mut tree).expect("root exists");
/// assert_eq!(root_ref.pop_item(), Ok("banana"));
/// assert_eq!(root_ref.pop_item(), Err(PopError::Empty(root.clone().into())));
/// // unblock "child_blocked"
/// child_blocked.try_ref(&mut tree)
///     .expect("child_blocked exists")
///     .set_weight(2);
/// let child_unblocked = child_blocked;
/// child_unblocked.try_ref(&mut tree)
///     .expect("child_unblocked exists")
///     .push_item("cashews");
/// let mut root_ref = root.try_ref(&mut tree).expect("root exists");
// TODO
// /// assert_eq!(root_ref.pop_item(), Ok("apple"));
// /// assert_eq!(root_ref.pop_item(), Ok("cashews"));
// /// assert_eq!(root_ref.pop_item(), Err(PopError::Empty(root.clone().into())));
/// ```
///
#[derive(Debug)]
pub struct Tree<T, F> {
    root: Node<T, F>,
    sequence_counter: node::SequenceCounter,
}
impl<T, F> Tree<T, F> {
    /// Creates a tree with a single root node
    #[must_use]
    pub fn new() -> Self {
        let (root, sequence_counter) = Node::new_root();
        Tree {
            root,
            sequence_counter,
        }
    }
    /// Returns the [`NodeId`] of the root node
    pub fn root_id(&self) -> NodeId<ty::Root> {
        #![allow(clippy::unused_self)]
        id::ROOT
    }
    //TODO: remove this non-external getter
    fn get_node<'a, P>(&self, node_path: &'a P) -> Result<&Node<T, F>, InvalidNodePath>
    where
        &'a P: Into<&'a [NodePathElem]>,
    {
        self.root.get_child(node_path.into())
    }
    //TODO: remove this non-external getter
    fn get_node_mut<'a, P>(&mut self, node_path: &'a P) -> Result<&mut Node<T, F>, InvalidNodePath>
    where
        &'a P: Into<&'a [NodePathElem]>,
    {
        self.root.get_child_mut(node_path.into())
    }
    /// Removes an empty node
    ///
    /// **Note:** Explicit [`NodeId`] is required to preserve idempotency.
    /// E.g. Removing a node may change the path of adjacent nodes.
    ///
    /// # Errors
    /// Returns an error if the specified `NodeId` does not point to a valid node,
    ///  or if the node has existing children.
    ///
    pub fn remove_node(&mut self, node_id: &NodeId<ty::Child>) -> Result<(), RemoveError> {
        let node_id_cloned = NodePath::from(node_id.clone());
        let (parent_id, last_elem) = node_id_cloned.parent();
        let mut parent = parent_id.try_ref(self)?;
        parent
            .remove_child(last_elem, node_id)
            .map(|_| ())
            .map_err(|e| e.attach_id(node_id))
    }
    /// Calculate the total node count (including the root)
    pub fn sum_node_count(&self) -> usize {
        self.root.sum_node_count()
    }
}
impl<T, F> Default for Tree<T, F> {
    fn default() -> Self {
        Self::new()
    }
}

pub use refs::{NodeRefMut, NodeRefMutWeighted};
mod refs {
    use crate::error::{InvalidNodePath, RemoveErrorInner};
    use crate::id::{
        ty, NodeId, NodeIdTyped, NodePath, NodePathElem, NodePathRefTyped, NodePathTyped,
        SequenceSource,
    };
    use crate::node::{self, Node, NodeInfoIntrinsic};
    use crate::order::Type as OrderType;
    use crate::weight_vec;
    use crate::{PopError, Tree, Weight};

    /// Mutable reference to a [`Node`]
    #[must_use]
    pub struct NodeRefMut<'tree, 'path, T, F> {
        node: &'tree mut Node<T, F>,
        path: NodePathRefTyped<'path>,
        sequence_counter: &'tree mut node::SequenceCounter,
    }
    impl<'tree, 'path, T, F> NodeRefMut<'tree, 'path, T, F> {
        /// Adds an empty child node, with optional weight
        pub fn add_child(&mut self, weight: Option<Weight>) -> NodeId<ty::Child> {
            let (child_part, child_node) = self.node.add_child(weight, self.sequence_counter);
            let path = self.path.clone_inner().append(child_part);
            path.with_sequence(child_node)
        }
        /// Mutable access to filter
        pub fn filter(&mut self) -> &mut Option<F> {
            &mut self.node.filter
        }
        /// Appends an item to the queue
        pub fn push_item(&mut self, item: T) {
            self.node.queue.push_back(item);
        }
        /// Pops an item from the queue
        ///
        /// # Errors
        /// Returns an error if the pop failed
        ///
        pub fn pop_item(&mut self) -> Result<T, PopError<NodeIdTyped>> {
            self.node
                .pop_item()
                .map_err(|e| e.map_inner(|()| self.path.clone_inner().with_sequence(self.node)))
        }
        /// Sets the [`OrderType`]
        pub fn set_order(&mut self, order: OrderType) {
            self.node.set_order(order);
        }
        pub(super) fn remove_child<S: SequenceSource>(
            &mut self,
            id_elem: NodePathElem,
            sequence_source: &S,
        ) -> Result<(Weight, Node<T, F>), RemoveErrorInner> {
            self.node.remove_child(id_elem, sequence_source)
        }
        pub(crate) fn overwrite_from(&mut self, info: NodeInfoIntrinsic<T, F>) {
            self.node.overwrite_from(info);
        }
    }

    /// Mutable reference to a [`Node`] with an associated [`Weight`]
    #[must_use]
    pub struct NodeRefMutWeighted<'tree, 'path, T, F> {
        weight_ref: weight_vec::RefMutWeight<'tree, 'tree>,
        inner: NodeRefMut<'tree, 'path, T, F>,
    }
    impl<'tree, 'path, T, F> NodeRefMutWeighted<'tree, 'path, T, F> {
        /// Sets the weight
        pub fn set_weight(&mut self, weight: Weight) {
            self.weight_ref.set_weight(weight);
        }
        /// Gets the weight
        #[must_use]
        pub fn get_weight(&self) -> Weight {
            self.weight_ref.get_weight()
        }
        /// Downgrades to [`NodeRefMut`]
        pub fn into_inner(self) -> NodeRefMut<'tree, 'path, T, F> {
            self.inner
        }
    }
    impl<'tree, 'path, T, F> std::ops::Deref for NodeRefMutWeighted<'tree, 'path, T, F> {
        type Target = NodeRefMut<'tree, 'path, T, F>;
        fn deref(&self) -> &Self::Target {
            &self.inner
        }
    }
    impl<T, F> std::ops::DerefMut for NodeRefMutWeighted<'_, '_, T, F> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.inner
        }
    }
    impl NodePath<ty::Root> {
        /// Returns `NodeRefMut` within the specified `Tree`
        ///
        /// # Errors
        /// Returns an error if the specified `NodeId` does not point to a valid node
        ///
        pub fn try_ref<'tree, T, F>(
            &self,
            tree: &'tree mut Tree<T, F>,
        ) -> Result<NodeRefMut<'tree, '_, T, F>, InvalidNodePath> {
            let path = self.into();
            let Tree {
                root,
                sequence_counter,
            } = tree;
            Ok(NodeRefMut {
                node: root,
                path,
                sequence_counter,
            })
        }
    }
    impl NodePath<ty::Child> {
        /// Returns `NodeRefMutWeighted` within the specified `Tree`
        ///
        /// # Errors
        /// Returns an error if the specified `NodeId` does not point to a valid **child** node
        ///
        pub fn try_ref<'tree, T, F>(
            &self,
            tree: &'tree mut Tree<T, F>,
        ) -> Result<NodeRefMutWeighted<'tree, '_, T, F>, InvalidNodePath> {
            let path = self;
            let ref_else_root_node = tree
                .root
                .get_child_and_weight_parent_order_mut(path.into())?;
            let (weight_ref, node) = ref_else_root_node.map_err(|_| path.clone())?;
            let path = path.into();
            let sequence_counter = &mut tree.sequence_counter;
            Ok(NodeRefMutWeighted {
                weight_ref,
                inner: NodeRefMut {
                    node,
                    path,
                    sequence_counter,
                },
            })
        }
    }
    impl NodePathTyped {
        /// Returns `NodeRefMut` within the specified `Tree`
        ///
        /// # Errors
        /// Returns an error if the specified `NodeId` does not point to a valid node
        ///
        pub fn try_ref<'tree, T, F>(
            &self,
            tree: &'tree mut Tree<T, F>,
        ) -> Result<NodeRefMut<'tree, '_, T, F>, InvalidNodePath> {
            match self {
                Self::Root(path) => path.try_ref(tree),
                Self::Child(path) => path.try_ref(tree).map(NodeRefMutWeighted::into_inner),
            }
        }
    }
    impl NodeIdTyped {
        /// Returns `NodeRefMut` to the specified `NodeId`
        ///
        /// # Errors
        /// Returns an error if the specified `NodeId` does not point to a valid node
        ///
        pub fn try_ref<'path, 'tree, T, F>(
            &'path self,
            tree: &'tree mut Tree<T, F>,
        ) -> Result<NodeRefMut<'tree, '_, T, F>, InvalidNodePath> {
            match self {
                Self::Root(id) => id.try_ref(tree),
                Self::Child(id) => id.try_ref(tree).map(NodeRefMutWeighted::into_inner),
            }
        }
    }
}
