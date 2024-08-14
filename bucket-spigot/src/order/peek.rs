// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use super::{source::OrderSource as _, CountsRemaining, OrderNode, RandResult, Root};
use crate::{child_vec::ChildVec, Child, Network};
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
    ) -> RandResult<Peeked<'a, T>> {
        let root = &self.root;
        let mut root_order = self.root_order.0.clone();
        let mut root_remaining = CountsRemaining::new(root.len());

        let mut effort_count = 0;

        let mut chosen_elems = Vec::with_capacity(peek_len.min(64));
        for _ in 0..peek_len {
            let (elem, effort) = peek_inner(rng, root, &mut root_order, &mut root_remaining)?;
            effort_count += effort;
            if let Some(elem) = elem {
                chosen_elems.push(elem);
            } else {
                break;
            }
        }

        Ok(Peeked {
            items: chosen_elems,
            root_order: Root(root_order),
            effort_count,
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
    current: &'a ChildVec<Child<T, U>>,
    order_node: &mut OrderNode,
    current_remaining: &mut CountsRemaining,
) -> RandResult<(Option<&'a T>, u64)>
where
    R: rand::Rng + ?Sized,
{
    let order_current = &mut order_node.order;
    let order_children = &mut order_node.children;

    let mut effort_count = 0;

    while !current_remaining.is_fully_exhausted() {
        assert_eq!(current.len(), order_children.len());
        assert_eq!(current.len(), current_remaining.child_count_if_nonempty());

        let child_index = order_current
            .next_in(rng, current)
            .expect("current should not be empty")?;

        let remaining_slot = current_remaining.child_mut(child_index);
        if remaining_slot.is_none() {
            // chosen child is known to to be exhausted
            continue;
        }

        #[allow(clippy::panic)]
        let Some(child_node) = current.children().get(child_index) else {
            panic!("valid current.children index ({child_index}) from order")
        };
        #[allow(clippy::panic)]
        let Some(child_order) = order_children.get_mut(child_index) else {
            panic!("valid order_children index ({child_index}) from order")
        };

        // effort: lookup child_node and child_order
        effort_count += 1;

        let elem = match child_node {
            Child::Bucket(bucket) => {
                let bucket_items = &bucket.items;
                if bucket_items.is_empty() {
                    None
                } else {
                    let elem_index = Rc::make_mut(child_order)
                        .order
                        .next_in_equal(rng, bucket_items)
                        .expect("bucket should not be empty")?;
                    #[allow(clippy::panic)]
                    let Some(elem) = bucket_items.get(elem_index) else {
                        panic!("valid bucket_items index ({elem_index}) from order")
                    };

                    // effort: lookup bucket element
                    effort_count += 1;

                    Some(elem)
                }
            }
            Child::Joint(joint) => {
                if joint.next.is_empty() {
                    None
                } else if let Some(remaining) = remaining_slot {
                    let (elem, child_effort_count) = peek_inner(
                        rng,
                        &joint.next,
                        Rc::make_mut(child_order),
                        remaining.as_mut_or_init(|| CountsRemaining::new(joint.next.len())),
                    )?;

                    // effort: recursion effort
                    effort_count += child_effort_count;

                    elem
                } else {
                    None
                }
            }
        };
        if let Some(elem) = elem {
            return Ok((Some(elem), effort_count));
        }
        current_remaining.set_empty(child_index);
    }
    Ok((None, effort_count))
}

/// Resulting items and tentative ordering state from [`Network::peek`]
pub struct Peeked<'a, T> {
    // TODO include metadata for which node the item came from
    items: Vec<&'a T>,
    root_order: Root,
    effort_count: u64,
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
    #[allow(unused)]
    /// For tests only, return the amount of effort required for this peek result
    pub(crate) fn get_effort_count(&self) -> u64 {
        self.effort_count
    }
}
/// Resulting tentative ordering state from [`Network::peek`] to apply in
/// [`Network::finalize_peeked`]
#[must_use]
#[allow(clippy::module_name_repetitions)]
pub struct PeekAccepted {
    new_root_order: Root,
}
