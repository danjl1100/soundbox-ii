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
    #[allow(clippy::missing_panics_doc)] // TODO
    pub fn peek<'a, R: rand::Rng + ?Sized>(
        &'a self,
        #[allow(unused)] // TODO
        rng: &mut R,
        peek_len: usize,
    ) -> Result<Peeked<'a, T>, rand::Error> {
        let mut chosen = vec![];

        let child_nodes = &self.root;
        let mut root_order = self.root_order.0.clone();

        let child_orders = &mut root_order.children;

        let order = &mut root_order.order;
        let mut remaining = CountsRemaining::new(child_nodes.len());

        let mut debug_count = 0; // DEBUG TRAINING WHEELS
        while chosen.len() < peek_len {
            debug_count += 1; // DEBUG TRAINING WHEELS
            assert!(debug_count < 100); // DEBUG TRAINING WHEELS

            if remaining.is_empty() || child_nodes.is_empty() {
                // TODO remove early return when implementing depth traversal
                return Ok(Peeked {
                    items: vec![],
                    root_order: Root(root_order),
                });
            }

            assert_eq!(child_nodes.len(), child_orders.len());

            let child_index = order
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

            let is_empty = match child_node {
                Child::Bucket(bucket_elems) => {
                    let is_empty = bucket_elems.is_empty();
                    if !is_empty {
                        let elem_index = Rc::make_mut(child_order)
                            .order
                            .next_in(rng, bucket_elems)
                            .expect("bucket should not be empty");
                        #[allow(clippy::panic)]
                        let Some(elem) = bucket_elems.get(elem_index) else {
                            panic!("valid bucket_elems index ({elem_index}) from order")
                        };
                        chosen.push(elem);
                    }
                    is_empty
                }
                Child::Joint(_) => todo!(),
            };
            if is_empty {
                remaining.set_empty(child_index);
            }
        }
        Ok(Peeked {
            items: chosen,
            root_order: Root(root_order),
        })
    }
    /// Finalizes the specified [`Peeked`], advancing the network state (if any)
    pub fn finalize_peeked(&mut self, peeked: PeekAccepted) {
        let PeekAccepted { new_root_order } = peeked;
        self.root_order = new_root_order;
    }
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

struct CountsRemaining(Vec<Option<()>>);
impl CountsRemaining {
    fn new(len: usize) -> Self {
        Self(vec![Some(()); len])
    }
    fn set_empty(&mut self, index: usize) {
        self.0[index].take();

        // check if all is exhausted
        if self.0.iter().all(Option::is_none) {
            self.0.clear();
        }
    }
    fn is_empty(&self) -> bool {
        self.0.is_empty()
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
