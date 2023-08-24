// Copyright (C) 2021-2023  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Iterator functionality for [`Tree`]
//!
//! See specific functions for details:
//!  * Mutable iterators - [`Tree::enumerate_mut`], [`Tree::enumerate_mut_subtree`],
//!      [`Tree::enumerate_mut_filters`], and [`Tree::enumerate_mut_subtree_filters`]
//!  * Shared/reference iterators - [`Tree::iter_ids`] and [`Tree::enumerate`]
#![allow(clippy::module_name_repetitions)]

#[allow(unused_imports)] // for doc comments, above
use crate::Tree;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod tests_mut;

mod shared_ref {
    use crate::{
        id::{NodeIdTyped, NodePathElem, NodePathTyped},
        node::Children,
        Node, Tree,
    };
    impl<T, F> Tree<T, F> {
        /// Creates a depth-first iterator over [`NodeIdTyped`]s
        pub fn iter_ids(&self) -> impl Iterator<Item = NodeIdTyped> + '_ {
            self.enumerate().map(|(id, _)| id)
        }
        /// Creates a depth-first iterator over [`NodeIdTyped`]s and [`Node`]s
        pub fn enumerate(&self) -> impl Iterator<Item = (NodeIdTyped, &'_ Node<T, F>)> + '_ {
            Iter {
                parent_idxs: vec![],
                next: Some(&self.root),
            }
        }
    }
    /// Depth-first iterator over [`NodeIdTyped`]s and [`Node`]s
    struct Iter<'a, T, F> {
        /// Parent "Node and Index" pairs that lead to the tail node
        parent_idxs: Vec<(&'a Node<T, F>, NodePathElem)>,
        /// Next node to emit (with the child index to be explored)
        next: Option<&'a Node<T, F>>,
    }
    impl<'a, T, F> Iter<'a, T, F> {
        /// Collects `parent_idxs` into a [`NodePathTyped`]
        fn collect_parent_path(&self) -> NodePathTyped {
            self.parent_idxs.iter().map(|(_, idx)| *idx).collect()
        }
    }
    impl<'a, T, F> Iterator for Iter<'a, T, F> {
        type Item = (NodeIdTyped, &'a Node<T, F>);
        fn next(&mut self) -> Option<Self::Item> {
            let current_node = self.next.take()?;
            let current_node_id = self.collect_parent_path().with_sequence(current_node);
            self.next = {
                let mut last_idx = None;
                let mut parent_node = current_node;
                loop {
                    match &parent_node.children {
                        Children::Chain(chain) => {
                            let lookup_idx = last_idx.map_or(0, |x| x + 1);
                            if let Some((_, child_node)) = chain.nodes.get(lookup_idx) {
                                // found child
                                self.parent_idxs.push((parent_node, lookup_idx));
                                break Some(child_node);
                            }
                        }
                        Children::Items(_) => {}
                    }
                    if let Some((node, idx)) = self.parent_idxs.pop() {
                        // re-lookup parent
                        last_idx = Some(idx);
                        parent_node = node;
                        continue;
                    }
                    // no parents left to pop
                    break None;
                }
            };
            Some((current_node_id, current_node))
        }
    }
}

pub use mut_ref::{IterMut, IterMutBreadcrumb};
mod mut_ref {
    use crate::{error::InvalidNodePath, id::NodePathRefTyped, Node, Tree};

    pub use breadcrumb::IterMutBreadcrumb;
    mod breadcrumb;

    pub use wrapper::IterMut;
    mod wrapper;

    impl<T, F> Tree<T, F> {
        /// Creates a depth-first iterator-helper over [`NodePathRefTyped`]s and
        /// [`NodeRefMut`](`crate::refs::NodeRefMut`)s
        #[allow(clippy::missing_panics_doc)] // guaranteed by existence of root within Tree
        pub fn enumerate_mut(&mut self) -> impl IterMut<T, F> + '_ {
            wrapper::new(self, None).expect("valid root path")
        }
        /// Creates a depth-first iterator-helper over [`NodePathRefTyped`]s and
        /// [`NodeRefMut`](`crate::refs::NodeRefMut`)s
        /// for the subtree starting at the specified path
        ///
        /// # Errors
        /// Returns an error if the specified `limit_path` is invalid for this [`Tree`]
        pub fn enumerate_mut_subtree<'a>(
            &mut self,
            limit_path: impl Into<NodePathRefTyped<'a>>,
        ) -> Result<impl IterMut<T, F> + '_, InvalidNodePath> {
            wrapper::new(self, Some(limit_path.into()))
        }
    }
    impl<T, F: Clone> Tree<T, F> {
        /// Creates a depth-first iterator-helper over [`NodePathRefTyped`]s and
        /// [`NodeRefMut`](`crate::refs::NodeRefMut`)s
        #[allow(clippy::missing_panics_doc)] // guaranteed by existence of root within Tree
        pub fn enumerate_mut_filters(&mut self) -> impl IterMutBreadcrumb<T, F, F> + '_ {
            breadcrumb::Walker::new(self, None, Some(node_filter_clone)).expect("valid root path")
        }
        /// Creates a depth-first iterator-helper over [`NodePathRefTyped`]s and
        /// [`NodeRefMut`](`crate::refs::NodeRefMut`)s
        /// for the subtree starting at the specified path
        ///
        /// # Errors
        /// Returns an error if the specified `limit_path` is invalid for this [`Tree`]
        pub fn enumerate_mut_subtree_filters<'a>(
            &mut self,
            subtree_limit_path: impl Into<NodePathRefTyped<'a>>,
        ) -> Result<impl IterMutBreadcrumb<T, F, F> + '_, InvalidNodePath> {
            breadcrumb::Walker::new(
                self,
                Some(subtree_limit_path.into()),
                Some(node_filter_clone),
            )
        }
    }
    fn node_filter_clone<T, F: Clone>(node: &Node<T, F>) -> F {
        //TODO clone seems unavoidable for this setup... is it?
        node.filter.clone()
    }
}
