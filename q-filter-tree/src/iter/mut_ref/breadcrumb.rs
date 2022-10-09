// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Tracks iteration, along with a breadcrumb trail to the root node

use crate::{
    error::InvalidNodePath,
    id::{NodePathRefTyped, NodePathTyped},
    node::Children,
    refs::NodeRefMut,
    Node, Tree,
};

/// Interface for mutable iteration
pub trait IterMutBreadcrumb<T, F, B> {
    /// Performs the specified operation to all remaining elements
    ///
    /// # Arguments
    /// * `consume_fn` - the action to perform on all nodes
    ///     * `&[B]` - breadcrumb elements from the base node to the current node
    ///     * `NodePathRefTyped` - path for the current node
    ///     * `NodeRefMut` - current node
    ///
    /// # Errors
    /// Returns an error on the first occurrence of the `consume_fn` returning an error.
    /// Note this means the iteration may be interrupted at an arbitrary step.
    fn with_all<U, E>(&mut self, consume_fn: U) -> Result<(), E>
    where
        U: FnMut(&[B], NodePathRefTyped<'_>, NodeRefMut<'_, '_, T, F>) -> Result<(), E>;
    /// Performs the specified operation to the next yielded element
    ///
    /// # Arguments
    /// * `consume_fn` - the action to perform on all nodes
    ///     * `&[B]` - breadcrumb elements from the base node to the current node
    ///     * `NodePathRefTyped` - path for the current node
    ///     * `NodeRefMut` - current node
    fn with_next<U, V>(&mut self, consume_fn: U) -> Option<V>
    where
        U: FnOnce(&[B], NodePathRefTyped<'_>, NodeRefMut<'_, '_, T, F>) -> V;
}
// NOTE
// Ideally, want an Iterator that yields (&[&F], NodePathTyped, &mut OrderVec<T>)
// BUT this is impossible in Safe Rust  (compiler cannot prove that each returned &mut is non-overlapping)
//
// Instead, redefine as a "next" function that accepts a closure of what-to-do.

/// Iterator-like helper for depth-first traversal over [`NodePathRefTyped`]s and [`NodeRefMut`]s
/// from a [`Tree`].
///
/// Created by [`Tree::enumerate_mut_filters`] and [`Tree::enumerate_mut_subtree_filters`].
pub(super) struct Walker<'tree, T, F, W, B>
where
    W: Fn(&Node<T, F>) -> B,
{
    tree: &'tree mut Tree<T, F>,
    limit_path_length: usize,
    breadcrumb_state: Option<BreadcrumbState<W, B>>,
    current_path: Option<NodePathTyped>,
}
struct BreadcrumbState<W, B> {
    breadcrumb: Vec<B>,
    breadcrumb_fn: W,
}
impl<'tree, T, F, W, B: Clone> Walker<'tree, T, F, W, B>
where
    W: Fn(&Node<T, F>) -> B,
{
    /// Attempts to create a new `Walker` iterator instance
    ///
    /// Returns an error if the specified `limit_path` is invalid for this [`Tree`]
    pub fn new<'a>(
        tree: &'tree mut Tree<T, F>,
        subtree_limit_path: Option<NodePathRefTyped<'a>>,
        breadcrumb_fn: Option<W>,
    ) -> Result<Self, InvalidNodePath> {
        let root = tree.root_id();
        let limit_path = subtree_limit_path.unwrap_or_else(|| NodePathRefTyped::from(&root));
        let (start_path, breadcrumb_state) = if let Some(breadcrumb_fn) = breadcrumb_fn {
            let mut breadcrumb = Vec::with_capacity(limit_path.elems().len() + 1);
            let mut path = NodePathTyped::from(*tree.root_id());
            for &elem in limit_path.elems() {
                let (_, node) = path.try_ref_shared(tree)?;
                breadcrumb.push(breadcrumb_fn(node));
                path = path.append(elem).into();
            }
            let breadcrumb_state = BreadcrumbState {
                breadcrumb,
                breadcrumb_fn,
            };
            (path, Some(breadcrumb_state))
        } else {
            let path = limit_path.clone_inner();
            (path, None)
        };
        assert_eq!(start_path.as_ref(), limit_path);
        Ok(Self {
            tree,
            limit_path_length: limit_path.elems().len(),
            breadcrumb_state,
            current_path: Some(start_path),
        })
    }
}
impl<'tree, T, F, W, B: Clone> IterMutBreadcrumb<T, F, B> for Walker<'tree, T, F, W, B>
where
    W: Fn(&Node<T, F>) -> B,
{
    fn with_all<U, E>(&mut self, mut consume_fn: U) -> Result<(), E>
    where
        U: FnMut(&[B], NodePathRefTyped<'_>, NodeRefMut<'_, '_, T, F>) -> Result<(), E>,
    {
        while let Some(result) = self.with_next(&mut consume_fn) {
            match result {
                Err(err) => return Err(err),
                Ok(()) => continue,
            }
        }
        Ok(())
    }
    fn with_next<U, V>(&mut self, consume_fn: U) -> Option<V>
    where
        U: FnOnce(&[B], NodePathRefTyped<'_>, NodeRefMut<'_, '_, T, F>) -> V,
    {
        const INVALID_INDEX: &str = "valid index from internal Walker iterator state";
        if let Some(current_path) = self.current_path.take() {
            // execute `consume_fn` for current node
            let result = {
                let node = current_path.try_ref(self.tree).expect(INVALID_INDEX);
                let path = (&current_path).into();
                let breadcrumb = if let Some(state) = &mut self.breadcrumb_state {
                    state.breadcrumb.push((state.breadcrumb_fn)(&node));
                    &state.breadcrumb[..]
                } else {
                    &[]
                };
                consume_fn(breadcrumb, path, node)
            };
            // calculate next path
            let mut popped_count = 0;
            self.current_path = {
                let mut next_path = current_path;
                let mut last_idx = None;
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
                        // mark additional `pop` required for `breadcrumb`
                        popped_count += 1;
                        continue;
                    }
                    // no parents left to pop
                    break None;
                }
            };
            if let Some(state) = &mut self.breadcrumb_state {
                // apply pop to `breadcrumb`, for next iteration
                state
                    .breadcrumb
                    .truncate(state.breadcrumb.len().saturating_sub(popped_count));
            }
            // return the result
            Some(result)
        } else {
            None
        }
    }
}
