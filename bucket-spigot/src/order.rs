// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

//! Ordering for selecting child nodes and child items throughout the [`Network`]

use crate::{Child, Network};
use std::rc::Rc;

impl<T, U> Network<T, U> {
    /// Returns a proposed sequence of items leaving the spigot.
    ///
    /// NOTE: Need to finalize the peeked items to progress the [`Network`] state beyond those
    /// peeked items (depending on the child-ordering involved)
    ///
    /// # Errors
    /// Returns any errors reported by the provided [`rand::Rng`] instance
    ///
    /// # Panics
    /// Panics if the internal order state does not match the item node structure
    pub fn peek<'a, R: rand::Rng + ?Sized>(
        &'a self,
        rng: &mut R,
        peek_len: usize,
    ) -> Result<Peeked<'a, T>, rand::Error> {
        let child_nodes = &self.root;
        let mut root_order = self.root_order.0.clone();
        let mut root_remaining = CountsRemaining::new(child_nodes.len());

        let chosen_elems = std::iter::from_fn(|| {
            peek_inner(rng, child_nodes, &mut root_order, &mut root_remaining)
        })
        .take(peek_len)
        .collect();

        Ok(Peeked {
            items: chosen_elems,
            root_order: Root(root_order),
        })
    }
    /// Finalizes the specified [`Peeked`], advancing the network state (if any)
    pub fn finalize_peeked(&mut self, peeked: PeekAccepted) {
        let PeekAccepted { new_root_order } = peeked;
        self.root_order = new_root_order;
    }
}

fn peek_inner<'a, R, T, U>(
    rng: &mut R,
    child_nodes: &'a [Child<T, U>],
    order_node: &mut node::Node,
    current_remaining: &mut CountsRemaining,
) -> Option<&'a T>
where
    R: rand::Rng + ?Sized,
{
    let current_order = &mut order_node.order;
    let child_orders = &mut order_node.children;

    if current_remaining.is_fully_exhausted() || child_nodes.is_empty() {
        return None;
    }

    assert_eq!(child_nodes.len(), child_orders.len());
    assert_eq!(
        child_nodes.len(),
        current_remaining.child_count_if_nonempty()
    );

    let child_index = current_order
        .next_in(rng, child_nodes)
        .expect("child_nodes should not be empty");

    #[allow(clippy::panic)]
    let Some(child_node) = child_nodes.get(child_index) else {
        panic!("valid child_nodes index ({child_index}) from order")
    };
    #[allow(clippy::panic)]
    let Some(child_order) = child_orders.get_mut(child_index) else {
        panic!("valid child_orders index ({child_index}) from order")
    };

    let elem = match child_node {
        Child::Bucket(bucket_elems) => {
            if bucket_elems.is_empty() {
                None
            } else {
                let elem_index = Rc::make_mut(child_order)
                    .order
                    .next_in(rng, bucket_elems)
                    .expect("bucket should not be empty");
                #[allow(clippy::panic)]
                let Some(elem) = bucket_elems.get(elem_index) else {
                    panic!("valid bucket_elems index ({elem_index}) from order")
                };
                Some(elem)
            }
        }
        Child::Joint(joint) => {
            let remaining_slot = current_remaining.child_mut(child_index);
            if joint.children.is_empty() {
                None
            } else if let Some(remaining) = remaining_slot {
                peek_inner(
                    rng,
                    &joint.children,
                    Rc::make_mut(child_order),
                    remaining.as_mut_or_init(|| CountsRemaining::new(joint.children.len())),
                )
            } else {
                None
            }
        }
    };
    // TODO does this need to be refactored to enable tail recursion?
    //      or does it not matter?
    if elem.is_none() {
        current_remaining.set_empty(child_index);
    }
    elem
}

/// Resulting items and tentative ordering state from [`Network::peek`]
pub struct Peeked<'a, T> {
    items: Vec<&'a T>,
    root_order: Root,
}
impl<'a, T> Peeked<'a, T> {
    /// Returns an the peeked items
    #[must_use]
    pub fn items(&self) -> &[&'a T] {
        &self.items
    }
    /// Cancels the peek operation and returns the referenced items
    #[must_use]
    pub fn cancel_into_items(self) -> Vec<&'a T> {
        self.items
    }
    /// Accepts the peeked items, discarding them to allow updating the original network
    pub fn accept_into_inner(self) -> PeekAccepted {
        PeekAccepted {
            new_root_order: self.root_order,
        }
    }
}
/// Resulting tentative ordering state from [`Network::peek`] to apply in
/// [`Network::finalize_peeked`]
#[must_use]
pub struct PeekAccepted {
    new_root_order: Root,
}

// `Option<Lazy<T>>` seems cleaner and more meaningful than `Option<Option<T>>`
// (heeding advice from the pedantic lint `clippy::option_option`)
#[derive(Clone, Default)]
enum Lazy<T> {
    Value(T),
    #[default]
    Uninit,
}
impl<T> Lazy<T> {
    fn as_mut(&mut self) -> Option<&mut T> {
        match self {
            Self::Value(value) => Some(value),
            Self::Uninit => None,
        }
    }
    fn as_mut_or_init(&mut self, init_fn: impl FnOnce() -> T) -> &mut T {
        match self {
            Self::Value(_) => {}
            Self::Uninit => {
                *self = Self::Value(init_fn());
            }
        }
        self.as_mut().expect("should initialize directly above")
    }
}

#[derive(Clone)]
struct CountsRemaining(Vec<Option<Lazy<Self>>>);
impl CountsRemaining {
    fn new(len: usize) -> Self {
        Self(vec![Some(Lazy::default()); len])
    }
    /// # Panics
    /// Panics if the index is out of bounds (greater than `len` provided in [`Self::new`]),
    /// or all children are exhausted.
    fn set_empty(&mut self, index: usize) {
        self.0[index].take();

        // check if all are exhausted
        if self.0.iter().all(Option::is_none) {
            // ensure any future calls error (loudly)
            self.0.clear();
        }
    }
    /// Returns a mutable reference to the child's remaining count (which may not yet be
    /// initialized) or `None` if the child is exhausted (e.g. via [`Self::set_empty`])
    ///
    /// # Panics
    /// Panics if the index is out of bounds (greater than `len` provided in [`Self::new`]),
    /// or all children are exhausted.
    fn child_mut(&mut self, index: usize) -> Option<&mut Lazy<Self>> {
        self.0[index].as_mut()
    }
    /// Returns true if all children are exhausted
    fn is_fully_exhausted(&self) -> bool {
        self.0.is_empty()
    }
    /// Returns the number of children, or `0` if all children are exhausted
    fn child_count_if_nonempty(&self) -> usize {
        self.0.len()
    }
}

trait OrderSource<R: rand::Rng + ?Sized> {
    /// Returns the next index in the order, within the range `0..=max_index`
    fn next(&mut self, rng: &mut R, max_index: usize) -> usize;
    /// Returns the next index in the order to index the specified `target` slice,
    /// or `None` if the specified `target` is empty.
    fn next_in<T>(&mut self, rng: &mut R, target: &[T]) -> Option<usize> {
        let max_index = target.len().checked_sub(1)?;
        let next = self.next(rng, max_index);
        Some(next)
    }
}

pub(crate) use node::{Root, UnknownOrderPath};
mod node {
    //! Tree structure for [`Order`], meant to mirror the
    //! [`Network`](`crate::Network`) topology.

    use super::Order;
    use crate::path::{Path, PathRef};
    use std::rc::Rc;

    #[derive(Clone, Default, Debug)]
    pub(crate) struct Root(pub(super) Node);
    #[derive(Clone, Debug, Default)]
    pub struct Node {
        pub(super) order: Order,
        pub(super) children: Vec<Rc<Node>>,
    }

    impl Root {
        /// Adds a default node at the specified path.
        ///
        /// Returns the index of the new child on success.
        pub(crate) fn add(&mut self, path: PathRef<'_>) -> Result<usize, UnknownOrderPath> {
            let mut current_children = &mut self.0.children;

            for next_index in path {
                let Some(next_child) = current_children.get_mut(next_index) else {
                    return Err(UnknownOrderPath(path.clone_inner()));
                };
                current_children = &mut Rc::make_mut(next_child).children;
            }

            let new_index = current_children.len();

            current_children.push(Rc::new(Node::default()));

            Ok(new_index)
        }
    }

    /// The specified path does not match an order-node
    #[derive(Debug)]
    pub struct UnknownOrderPath(pub(crate) Path);
}

#[derive(Clone, Debug)]
enum Order {
    InOrder(InOrder),
}
impl Default for Order {
    fn default() -> Self {
        Self::InOrder(InOrder::default())
    }
}
impl<R: rand::Rng + ?Sized> OrderSource<R> for Order {
    fn next(&mut self, rng: &mut R, max_index: usize) -> usize {
        match self {
            Order::InOrder(inner) => inner.next(rng, max_index),
        }
    }
}

#[derive(Clone, Debug, Default)]
struct InOrder(usize);
impl<R: rand::Rng + ?Sized> OrderSource<R> for InOrder {
    fn next(&mut self, _rng: &mut R, max_index: usize) -> usize {
        let next = if self.0 > max_index { 0 } else { self.0 };
        self.0 = next.wrapping_add(1);
        next
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_order() {
        let mut uut = InOrder::default();
        let rng = &mut crate::tests::PanicRng;

        assert_eq!(uut.next(rng, 5), 0);
        assert_eq!(uut.next(rng, 5), 1);
        assert_eq!(uut.next(rng, 5), 2);
        assert_eq!(uut.next(rng, 5), 3);
        assert_eq!(uut.next(rng, 5), 4);
        assert_eq!(uut.next(rng, 5), 5);
        //
        assert_eq!(uut.next(rng, 5), 0);
        //
        assert_eq!(uut.next(rng, 2), 1);
        assert_eq!(uut.next(rng, 2), 2);
        assert_eq!(uut.next(rng, 2), 0);
        assert_eq!(uut.next(rng, 2), 1);
        //
        assert_eq!(uut.next(rng, 1), 0);
        //
        assert_eq!(uut.next(rng, 0), 0);
        assert_eq!(uut.next(rng, 0), 0);
    }
}
