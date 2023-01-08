// Copyright (C) 2021-2023  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Wrapper for [`super::breadcrumb`] types, without the breadcrumb

use super::{breadcrumb, IterMutBreadcrumb};
use crate::{error::InvalidNodePath, id::NodePathRefTyped, refs::NodeRefMut, Node, Tree};
use shared::Never;

/// Interface for mutable iteration
pub trait IterMut<T, F>: IterMutBreadcrumb<T, F, Never> {
    /// Performs the specified operation to all remaining elements
    ///
    /// # Arguments
    /// * `consume_fn` - the action to perform on all nodes
    ///     * `NodePathRefTyped` - path for the current node
    ///     * `NodeRefMut` - current node
    ///
    /// # Errors
    /// Returns an error on the first occurrence of the `consume_fn` returning an error.
    /// Note this means the iteration may be interrupted at an arbitrary step.
    fn with_all<U, E>(&mut self, mut consume_fn: U) -> Result<(), E>
    where
        U: FnMut(NodePathRefTyped<'_>, NodeRefMut<'_, '_, T, F>) -> Result<(), E>,
    {
        IterMutBreadcrumb::with_all(self, |_, path, node| consume_fn(path, node))
    }
    /// Performs the specified operation to the next yielded element
    ///
    /// # Arguments
    /// * `consume_fn` - the action to perform on all nodes
    ///     * `NodePathRefTyped` - path for the current node
    ///     * `NodeRefMut` - current node
    fn with_next<U, V>(&mut self, consume_fn: U) -> Option<V>
    where
        U: FnOnce(NodePathRefTyped<'_>, NodeRefMut<'_, '_, T, F>) -> V,
    {
        IterMutBreadcrumb::with_next(self, |_, path, node| consume_fn(path, node))
    }
}
pub(super) struct Wrapper<'tree, T, F, W>(breadcrumb::Walker<'tree, T, F, W, Never>)
where
    W: Fn(&Node<T, F>) -> Never;
/// Attempts to create a new `Walker` iterator instance
///
/// Returns an error if the specified `limit_path` is invalid for this [`Tree`]
// NOTE: free-standing function, since impl block doesn't allow impl-bounds as trait bounds (right?)
pub(super) fn new<'a, 'tree, T, F>(
    tree: &'tree mut Tree<T, F>,
    subtree_limit_path: Option<NodePathRefTyped<'a>>,
) -> Result<Wrapper<'tree, T, F, impl Fn(&Node<T, F>) -> Never>, InvalidNodePath> {
    let no_fn = false.then_some(|_: &Node<T, F>| unreachable!());
    breadcrumb::Walker::new(tree, subtree_limit_path, no_fn).map(Wrapper)
}
impl<'tree, T, F, W> IterMutBreadcrumb<T, F, Never> for Wrapper<'tree, T, F, W>
where
    W: Fn(&Node<T, F>) -> Never,
{
    fn with_all<U, E>(&mut self, consume_fn: U) -> Result<(), E>
    where
        U: FnMut(&[Never], NodePathRefTyped<'_>, NodeRefMut<'_, '_, T, F>) -> Result<(), E>,
    {
        self.0.with_all(consume_fn)
    }

    fn with_next<U, V>(&mut self, consume_fn: U) -> Option<V>
    where
        U: FnOnce(&[Never], NodePathRefTyped<'_>, NodeRefMut<'_, '_, T, F>) -> V,
    {
        self.0.with_next(consume_fn)
    }
}
impl<'tree, T, F, W> IterMut<T, F> for Wrapper<'tree, T, F, W> where W: Fn(&Node<T, F>) -> Never {}
