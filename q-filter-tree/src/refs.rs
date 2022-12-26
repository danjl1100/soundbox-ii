// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Reference helpers for modifying [`Node`]s in the [`Tree`]
use crate::error::InvalidNodePath;
use crate::id::{
    ty, NodeId, NodeIdTyped, NodePath, NodePathRefTyped, NodePathTyped, Sequence, SequenceSource,
};
use crate::node::meta::NodeInfoIntrinsic;
use crate::node::{self, Children, Node};
use crate::weight_vec::{self, OrderVec};
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
    /// Returns a mut handle to the child items, if the node is type items (not chain)
    pub fn child_items(&mut self) -> Option<&mut OrderVec<T>> {
        match &mut self.node.children {
            Children::Items(items) => Some(items),
            Children::Chain(_) => None,
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
impl<'tree, 'path, T, F> SequenceSource for NodeRefMut<'tree, 'path, T, F> {
    fn sequence(&self) -> Sequence {
        self.node.sequence()
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
    const DEFAULT_WEIGHT: Weight = 1;
    /// Adds an empty node with the specified filter
    pub fn add_child_filter(&mut self, filter: F) -> NodeId<ty::Child> {
        let weight = Self::DEFAULT_WEIGHT;
        let info = NodeInfoIntrinsic::default_with_filter(filter);
        self.add_child_from(weight, info)
    }
}
impl<'tree, 'path, T, F> NodeChildrenRefMut<'tree, 'path, T, F>
where
    F: Default,
{
    /// Adds an empty child node, with the default weight
    pub fn add_child_default(&mut self) -> NodeId<ty::Child> {
        self.add_child_from(Self::DEFAULT_WEIGHT, NodeInfoIntrinsic::default())
    }
    /// Adds an empty child node, with optional weight
    pub fn add_child(&mut self, weight: Weight) -> NodeId<ty::Child> {
        self.add_child_from(weight, NodeInfoIntrinsic::default())
    }
}
impl<'tree, 'path, T, F> NodeChildrenRefMut<'tree, 'path, T, F> {
    /// Adds a node from the specified info, with default weight
    pub fn add_child_default_from(&mut self, info: NodeInfoIntrinsic<T, F>) -> NodeId<ty::Child> {
        self.add_child_from(Self::DEFAULT_WEIGHT, info)
    }
    /// Adds a node from the specified info, with specified weight
    pub fn add_child_from(
        &mut self,
        weight: Weight,
        info: NodeInfoIntrinsic<T, F>,
    ) -> NodeId<ty::Child> {
        let new_child = info.construct(self.sequence_counter);
        let child_path_part = self.node_children.nodes.len();
        let sequence = new_child.sequence_keeper();
        self.node_children.nodes.ref_mut().push((weight, new_child));
        let path = self.path.clone_inner().append(child_path_part);
        path.with_sequence(&sequence)
    }
    // TODO remove if not needed
    // /// Iterates the child nodes in order
    // pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Node<T, F>> {
    //     self.node_children.nodes.iter_mut_elems_straight()
    // }
}

/// Mutable reference to a node with an associated [`Weight`]
#[must_use]
pub struct NodeRefMutWeighted<'tree, 'path, T, F> {
    weight_ref: weight_vec::RefMutWeight<'tree, 'tree>,
    inner: NodeRefMut<'tree, 'path, T, F>,
}
impl<'tree, 'path, T, F> NodeRefMutWeighted<'tree, 'path, T, F> {
    /// Sets the weight, returning the old weight
    pub fn set_weight(&mut self, weight: Weight) -> Weight {
        self.weight_ref.set_weight(weight)
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
    pub fn try_ref<'tree, T, F>(
        &self,
        tree: &'tree mut impl AsMut<Tree<T, F>>,
    ) -> NodeRefMut<'tree, '_, T, F> {
        let tree = tree.as_mut();
        let path = self.into();
        let Tree {
            root,
            sequence_counter,
        } = tree;
        NodeRefMut {
            node: root,
            path,
            sequence_counter,
        }
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
        tree: &'tree mut impl AsMut<Tree<T, F>>,
    ) -> Result<NodeRefMutWeighted<'tree, '_, T, F>, InvalidNodePath> {
        let tree = tree.as_mut();
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
    /// Returns a the specified weight and `Node` reference within the `Tree`
    ///
    /// # Errors
    /// Returns an error if the [`NodePath`] is not a valid path for the specified `Tree`
    pub fn try_ref_shared<'tree, T, F>(
        &self,
        tree: &'tree Tree<T, F>,
    ) -> Result<(Weight, &'tree Node<T, F>), InvalidNodePath> {
        let path = self;
        match &tree.root.children {
            Children::Chain(chain) => chain.get_child_entry_shared(path.into()),
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
        tree: &'tree mut impl AsMut<Tree<T, F>>,
    ) -> Result<NodeRefMut<'tree, '_, T, F>, InvalidNodePath> {
        let tree = tree.as_mut();
        match self {
            Self::Root(path) => Ok(path.try_ref(tree)),
            Self::Child(path) => path.try_ref(tree).map(NodeRefMutWeighted::into_inner),
        }
    }
    /// Returns `Node` within the specified `Tree`
    ///
    /// # Errors
    /// Returns an error if the specified `NodeId` does not point to a valid node
    ///
    pub fn try_ref_shared<'tree, T, F>(
        &self,
        tree: &'tree Tree<T, F>,
    ) -> Result<(Option<Weight>, &'tree Node<T, F>), InvalidNodePath> {
        match self {
            Self::Root(_) => Ok((None, &tree.root)),
            Self::Child(path) => path
                .try_ref_shared(tree)
                .map(|(weight, node)| (Some(weight), node)),
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
        tree: &'tree mut impl AsMut<Tree<T, F>>,
    ) -> Result<NodeRefMut<'tree, '_, T, F>, InvalidNodePath> {
        let tree = tree.as_mut();
        match self {
            Self::Root(id) => Ok(id.try_ref(tree)),
            Self::Child(id) => id.try_ref(tree).map(NodeRefMutWeighted::into_inner),
        }
    }
}
