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

use error::InvalidNodePath;
pub mod error;

use id::{ty, NodeId, NodePath};
pub mod id;

pub use node::meta::NodeInfoIntrinsic as NodeInfo;
pub use node::Node;
mod node;

mod weight_vec;

use node::RemoveResult;
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
fn tree_add_to_doc_tests() {
    let mut tree: Tree<_, _> = Tree::new();
    let root = tree.root_id();
    //
    let mut root_ref = root.try_ref(&mut tree).expect("root exists");
    root_ref.filter = Some("filter value".to_string());
    let mut root_ref = root_ref.child_nodes().expect("root is chain");
    let child_blocked = root_ref.add_child(0);
    let child = root_ref.add_child_default();
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
    assert_eq!(root_ref.pop_item_queued(), Some("banana"));
    assert_eq!(root_ref.pop_item_queued(), None);
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
    assert_eq!(root_ref.pop_item_queued(), Some("apple"));
    assert_eq!(root_ref.pop_item_queued(), Some("cashews"));
    assert_eq!(root_ref.pop_item_queued(), None);
}
/// Tree data structure, consisting of nodes with queues of items `T`, filter `F`
///
/// # Example
/// ```
/// use q_filter_tree::{Tree, error::PopError};
/// let mut tree: Tree<_, _> = Tree::new();
/// let root = tree.root_id();
/// //
/// let mut root_ref = root.try_ref(&mut tree).expect("root exists");
/// root_ref.filter = Some("filter value".to_string());
/// let mut root_ref = root_ref.child_nodes().expect("root is chain");
/// let child_blocked = root_ref.add_child(0);
/// let child = root_ref.add_child(1);
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
/// assert_eq!(root_ref.pop_item_queued(), Some("banana"));
/// assert_eq!(root_ref.pop_item_queued(), None);
/// // unblock "child_blocked"
/// child_blocked.try_ref(&mut tree)
///     .expect("child_blocked exists")
///     .set_weight(2);
/// let child_unblocked = child_blocked;
/// child_unblocked.try_ref(&mut tree)
///     .expect("child_unblocked exists")
///     .push_item("cashews");
/// let mut root_ref = root.try_ref(&mut tree).expect("root exists");
/// assert_eq!(root_ref.pop_item_queued(), Some("apple"));
/// assert_eq!(root_ref.pop_item_queued(), Some("cashews"));
/// assert_eq!(root_ref.pop_item_queued(), None);
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
        Self::new_with_root(node::meta::NodeInfoIntrinsic::default())
    }
    /// Creates a tree with the specified root info
    pub fn new_with_root(node_info: node::meta::NodeInfoIntrinsic<T, F>) -> Self {
        let (root, sequence_counter) = node_info.construct_root();
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
    /// Removes an empty node
    ///
    /// **Note:** Explicit [`NodeId`] is required to preserve idempotency.
    /// E.g. Removing a node may change the path of adjacent nodes.
    ///
    /// # Errors
    /// Returns an error if the specified `NodeId` does not point to a valid node,
    ///  or if the node has existing children.
    ///
    pub fn remove_node(
        &mut self,
        node_id: &NodeId<ty::Child>,
    ) -> Result<RemoveResult<T, F, NodeId<ty::Child>>, InvalidNodePath> {
        let err_child_path_invalid = || InvalidNodePath::from(node_id.clone().into_inner());
        // calculate parent path
        let node_id_cloned = NodePath::from(node_id.clone());
        let (parent_id, last_elem) = node_id_cloned.into_parent();
        // remove child from parent
        let mut parent = parent_id.try_ref(self)?;
        match &mut parent.children {
            node::Children::Chain(chain) => chain
                .remove_child(last_elem, node_id)
                .map(|remove_result| remove_result.map_err(|e| e.map_id(|_| node_id.clone())))
                .map_err(|_| err_child_path_invalid()),
            node::Children::Items(_) => Err(err_child_path_invalid()),
        }
    }
    /// Calculate the total node count (including the root)
    pub fn sum_node_count(&self) -> usize {
        self.root.children.sum_node_count()
    }
    /// Pops an item from child node queues only (ignores items-leaf nodes)
    // ///
    // /// See: [`Self::pop_item`] for including items-leaf items for when `T: Copy`
    pub fn pop_item_queued(&mut self) -> Option<T> {
        self.root.pop_item_queued()
    }
}
// TODO reinstate (with tests)
// impl<T: Copy, F> Tree<T, F> {
//     /// Removes items from node queues, and finally copies from items-leaf node
//     pub fn pop_item(&mut self) -> Option<T> {
//         self.root.pop_item()
//     }
// }
impl<T, F> Default for Tree<T, F> {
    fn default() -> Self {
        Self::new()
    }
}

pub use refs::{NodeRefMut, NodeRefMutWeighted};
mod refs {
    use crate::error::InvalidNodePath;
    use crate::id::{
        ty, NodeId, NodeIdTyped, NodePath, NodePathRefTyped, NodePathTyped, SequenceSource,
    };
    use crate::node::meta::NodeInfoIntrinsic;
    use crate::node::{self, Children, Node};
    use crate::weight_vec;
    use crate::{Tree, Weight};

    /// Mutable reference to a node in the [`Tree`]
    #[must_use]
    pub struct NodeRefMut<'tree, 'path, T, F> {
        node: &'tree mut Node<T, F>,
        path: NodePathRefTyped<'path>,
        sequence_counter: &'tree mut node::SequenceCounter,
    }
    impl<'tree, 'path, T, F> NodeRefMut<'tree, 'path, T, F> {
        /// Returns a mut handle to the node-children, if the node is type chain (not items)
        pub fn child_nodes(&mut self) -> Option<NodeChildrenRefMut<'_, 'path, T, F>> {
            let Self {
                node,
                path,
                sequence_counter,
            } = self;
            match &mut node.children {
                Children::Chain(node_children) => Some(NodeChildrenRefMut {
                    node_children,
                    path: *path,
                    sequence_counter,
                }),
                Children::Items(_) => None,
            }
        }
    }
    impl<'tree, 'path, T, F> std::ops::Deref for NodeRefMut<'tree, 'path, T, F> {
        type Target = Node<T, F>;
        fn deref(&self) -> &Self::Target {
            self.node
        }
    }
    impl<'tree, 'path, T, F> std::ops::DerefMut for NodeRefMut<'tree, 'path, T, F> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            self.node
        }
    }

    /// Mutable reference to node-children in the [`Tree`]
    #[must_use]
    pub struct NodeChildrenRefMut<'tree, 'path, T, F> {
        node_children: &'tree mut node::Chain<T, F>,
        path: NodePathRefTyped<'path>,
        sequence_counter: &'tree mut node::SequenceCounter,
    }
    impl<'tree, 'path, T, F> NodeChildrenRefMut<'tree, 'path, T, F> {
        /// Adds an empty child node, with the default weight
        pub fn add_child_default(&mut self) -> NodeId<ty::Child> {
            const DEFAULT_WEIGHT: Weight = 1;
            self.add_child_from(DEFAULT_WEIGHT, None)
        }
        /// Adds an empty child node, with optional weight
        pub fn add_child(&mut self, weight: Weight) -> NodeId<ty::Child> {
            self.add_child_from(weight, None)
        }
        /// Adds an empty node from the (optional) specified info, with optional weight
        pub(crate) fn add_child_from(
            &mut self,
            weight: Weight,
            info: Option<NodeInfoIntrinsic<T, F>>,
        ) -> NodeId<ty::Child> {
            let new_child = info.unwrap_or_default().construct(self.sequence_counter);
            let child_path_part = self.node_children.nodes.len();
            let sequence = new_child.sequence_keeper();
            self.node_children.nodes.ref_mut().push((weight, new_child));
            let path = self.path.clone_inner().append(child_path_part);
            path.with_sequence(&sequence)
        }
    }

    /// Mutable reference to a node with an associated [`Weight`]
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
            match &mut tree.root.children {
                Children::Chain(chain) => {
                    let (weight_ref, node) = chain.get_child_entry_mut(path.into())?;
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
                Children::Items(_) => Err(path.clone().into()),
            }
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
