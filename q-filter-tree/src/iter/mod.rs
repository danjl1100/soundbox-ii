// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Iterator functionality for [`Tree`]
//!
//! See specific functions for details:
//!  * Mutable iterators - [`Tree::enumerate_mut`] and [`Tree::enumerate_mut_subtree`]
//!  * Shared/reference iterators - [`Tree::iter_ids`] and [`Tree::enumerate`]
use crate::{
    error::InvalidNodePath,
    id::{NodeIdTyped, NodePathElem, NodePathRefTyped, NodePathTyped},
    node::Children,
    refs::NodeRefMut,
    Node, Tree,
};

#[cfg(test)]
mod tests;

#[cfg(test)]
mod tests_mut;

impl<T, F> Tree<T, F> {
    /// Creates a depth-first iterator over [`NodeIdTyped`]s
    pub fn iter_ids(&self) -> impl Iterator<Item = NodeIdTyped> + '_ {
        self.enumerate().map(|(id, _)| id)
    }
    /// Creates a depth-first iterator over [`NodeIdTyped`]s and [`Node`]s
    pub(crate) fn enumerate(&self) -> impl Iterator<Item = (NodeIdTyped, &'_ Node<T, F>)> + '_ {
        IterIdSharedRefs {
            parent_idxs: vec![],
            next: Some(&self.root),
        }
    }
}
/// Depth-first iterator over [`NodeIdTyped`]s and [`Node`]s
struct IterIdSharedRefs<'a, T, F> {
    /// Parent "Node and Index" pairs that lead to the tail node
    parent_idxs: Vec<(&'a Node<T, F>, NodePathElem)>,
    /// Next node to emit (with the child index to be explored)
    next: Option<&'a Node<T, F>>,
}
impl<'a, T, F> IterIdSharedRefs<'a, T, F> {
    /// Collects `parent_idxs` into a [`NodePathTyped`]
    fn collect_parent_path(&self) -> NodePathTyped {
        self.parent_idxs.iter().map(|(_, idx)| *idx).collect()
    }
}
impl<'a, T, F> Iterator for IterIdSharedRefs<'a, T, F> {
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

impl<T, F: Clone> Tree<T, F> {
    /// Creates a depth-first iterator-helper over [`NodePathRefTyped`]s and [`NodeRefMut`]s
    pub fn enumerate_mut(&mut self) -> IterDetachedNodeMut<'_, T, F> {
        let root = self.root_id();
        self.enumerate_mut_subtree(&root).expect("valid root path")
    }
    /// Creates a depth-first iterator-helper over [`NodePathRefTyped`]s and [`NodeRefMut`]s
    /// for the subtree starting at the specified path
    ///
    /// # Errors
    /// Returns an error if the specified `limit_path` is invalid for this [`Tree`]
    pub fn enumerate_mut_subtree<'a, R>(
        &mut self,
        limit_path: R,
    ) -> Result<IterDetachedNodeMut<'_, T, F>, InvalidNodePath>
    where
        R: Into<NodePathRefTyped<'a>>,
    {
        IterDetachedNodeMut::new(self, limit_path.into())
    }
}

// NOTE
// Ideally, want an Iterator that yields (&[&F], NodePathTyped, &mut OrderVec<T>)
// BUT this is impossible in Safe Rust  (compiler cannot prove that each returned &mut is non-overlapping)
//
// Instead, redefine as a "next" function that accepts a closure of what-to-do.

/// Iterator-like helper for depth-first traversal over [`NodePathRefTyped`]s and [`NodeRefMut`]s
/// from a [`Tree`].
///
/// Created by [`Tree::enumerate_mut`] and [`Tree::enumerate_mut_subtree`].
#[allow(clippy::module_name_repetitions)]
pub struct IterDetachedNodeMut<'tree, T, F> {
    tree: &'tree mut Tree<T, F>,
    limit_path_length: usize,
    filter_args: Vec<F>,
    current_path: Option<NodePathTyped>,
}
impl<'tree, T, F: Clone> IterDetachedNodeMut<'tree, T, F> {
    /// Attempts to create a new `IterDetachedNodeMut` iterator instance
    ///
    /// Returns an error if the specified `limit_path` is invalid for this [`Tree`]
    fn new(
        tree: &'tree mut Tree<T, F>,
        limit_path: NodePathRefTyped<'_>,
    ) -> Result<Self, InvalidNodePath> {
        let (start_path, filter_args) = {
            let mut filter_args = Vec::with_capacity(limit_path.elems().len() + 1);
            let mut path = NodePathTyped::from(tree.root_id());
            for &elem in limit_path.elems() {
                let (_, node) = path.try_ref_shared(tree)?;
                filter_args.push(node.filter.clone()); //TODO clone seems unavoidable for this setup... is it?
                path = path.append(elem).into();
            }
            // assert_eq!(NodePathRefTyped::from(&path), limit_path);
            assert_eq!(path.as_ref(), limit_path);
            (path, filter_args)
        };
        Ok(Self {
            tree,
            limit_path_length: limit_path.elems().len(),
            filter_args,
            current_path: Some(start_path),
        })
    }
    /// Performs the specified operation to all remaining elements
    ///
    /// See [`with_next()`] for the closure arguments' description.
    ///
    /// [`with_next()`]: Self::with_next
    pub fn with_all<U>(&mut self, mut consume_fn: U)
    where
        U: FnMut(&[F], NodePathRefTyped<'_>, NodeRefMut<'_, '_, T, F>),
    {
        while self.with_next(&mut consume_fn).is_some() {
            continue;
        }
    }
    /// Performs the specified operation to the next yielded element
    ///
    /// # Arguments
    /// * `consume_fn` - the action to perform on all nodes
    ///     * `&[F]` - filter elements from the base node to the current node (prior to iteration)
    ///     * `NodePathRefTyped` - path for the current node
    ///     * `NodeRefMut` - current node
    pub fn with_next<U, V>(&mut self, consume_fn: U) -> Option<V>
    where
        U: FnOnce(&[F], NodePathRefTyped<'_>, NodeRefMut<'_, '_, T, F>) -> V,
    {
        const INVALID_INDEX: &str = "valid index from internal IterDetachedNodeMut iterator state";
        if let Some(current_path) = self.current_path.take() {
            let mut last_idx = None;
            let mut popped_count = 0;
            self.current_path = {
                let mut next_path = current_path.clone(); // TODO if possible, remove this clone
                loop {
                    let (_, parent_node) =
                        next_path.try_ref_shared(self.tree).expect(INVALID_INDEX);
                    let lookup_idx = last_idx.map_or(0, |x| x + 1);
                    match &parent_node.children {
                        Children::Chain(chain) => {
                            if let Some((_, _child_node)) = chain.nodes.get(lookup_idx) {
                                // found child
                                break Some(next_path.append(lookup_idx).into());
                            }
                        }
                        Children::Items(_) => {}
                    }
                    if next_path.elems().len() <= self.limit_path_length {
                        // reached end of the `limit_path`
                        break None;
                    }
                    if let NodePathTyped::Child(child_path) = next_path {
                        let (parent_path, idx) = child_path.into_parent();
                        // re-lookup parent
                        last_idx = Some(idx);
                        next_path = parent_path;
                        // mark additional `pop` required for `filter_args`
                        popped_count += 1;
                        continue;
                    }
                    // no parents left to pop
                    break None;
                }
            };
            // execute `consume_fn` for current node
            let current_node = current_path.try_ref(self.tree).expect(INVALID_INDEX);
            self.filter_args.push(current_node.filter.clone());
            let result = consume_fn(&self.filter_args, (&current_path).into(), current_node);
            // apply pop to `filter_args`, for next iteration
            self.filter_args
                .truncate(self.filter_args.len().saturating_sub(popped_count));
            // return the result
            Some(result)
        } else {
            None
        }
    }
}
