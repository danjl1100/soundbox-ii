// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use super::{CountsRemaining, OrderNode, Root, source::OrderSource as _};
use crate::{BucketId, Child, Network, child_vec::ChildVec, order::ArbitrarySource};
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
    pub fn peek<'a, R: ArbitrarySource>(
        &'a self,
        rng: &mut R,
        peek_len: usize,
    ) -> Result<Peeked<'a, T>, R::Error> {
        let root = &self.trees.item;
        let mut root_order = self.trees.order.0.clone();
        let mut root_remaining = CountsRemaining::new(root.len());

        let mut effort_count = 0;

        let capacity = peek_len.min(64); // TODO remove premature optimization? (no benchmarks?)
        let mut items = Vec::with_capacity(capacity);
        let mut source_buckets = Vec::with_capacity(capacity);
        for _ in 0..peek_len {
            let PeekResult {
                elem_bucket_id,
                effort_count: effort_this_peek,
            } = match peek_inner(rng, root, &mut root_order, &mut root_remaining)? {
                Ok(v) => v,
                Err(e) => {
                    #[expect(clippy::panic, reason = "report bug in `Order` implementation")]
                    {
                        panic!("{e}")
                    }
                }
            };
            effort_count += effort_this_peek;
            if let Some((elem, bucket_id)) = elem_bucket_id {
                items.push(elem);
                source_buckets.push(bucket_id);
            } else {
                break;
            }
        }

        Ok(Peeked {
            items,
            source_buckets,
            root_order: Root(root_order),
            effort_count,
        })
    }
    /// Finalizes the specified [`Peeked`], advancing the network state (if any)
    pub fn finalize_peeked(&mut self, peeked: PeekAccepted) {
        let PeekAccepted { new_root_order } = peeked;
        self.trees.order = new_root_order;
    }
}

struct OrderIndexError {
    order: super::Order,
    child_index: usize,
    target_len: usize,
}
impl std::fmt::Display for OrderIndexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            order,
            child_index,
            target_len,
        } = self;
        write!(
            f,
            "invalid order_children index ({child_index}) for target len ({target_len}) from order: {order:?}"
        )
    }
}

struct PeekResult<'a, T> {
    elem_bucket_id: Option<(&'a T, BucketId)>,
    effort_count: u64,
}

fn peek_inner<'a, R, T, U>(
    rng: &mut R,
    current: &'a ChildVec<Child<T, U>>,
    order_node: &mut OrderNode,
    current_remaining: &mut CountsRemaining,
) -> Result<Result<PeekResult<'a, T>, OrderIndexError>, R::Error>
where
    R: ArbitrarySource + ?Sized,
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

        let order_index_err = || {
            Ok(Err(OrderIndexError {
                order: order_current.clone(),
                child_index,
                target_len: current.len(),
            }))
        };

        let Some(child_node) = current.children().get(child_index) else {
            return order_index_err();
        };
        let Some(child_order) = order_children.get_mut(child_index) else {
            return order_index_err();
        };

        // effort: lookup child_node and child_order
        effort_count += 1;

        let elem = match child_node {
            Child::Bucket(bucket) => {
                let bucket_items = &bucket.items;
                if bucket_items.is_empty() {
                    None
                } else {
                    let child_order = &mut Rc::make_mut(child_order).order;
                    let elem_index = child_order
                        .next_in_equal(rng, bucket_items)
                        .expect("bucket should not be empty")?;
                    let Some(elem) = bucket_items.get(elem_index) else {
                        return Ok(Err(OrderIndexError {
                            order: child_order.clone(),
                            child_index,
                            target_len: bucket_items.len(),
                        }));
                    };

                    // effort: lookup bucket element
                    effort_count += 1;

                    Some((elem, bucket.id))
                }
            }
            Child::Joint(joint) => {
                if joint.next.is_empty() {
                    None
                } else if let Some(remaining) = remaining_slot {
                    let peek_result = peek_inner(
                        rng,
                        &joint.next,
                        Rc::make_mut(child_order),
                        remaining.as_mut_or_init(|| CountsRemaining::new(joint.next.len())),
                    )?;
                    let PeekResult {
                        elem_bucket_id,
                        effort_count: child_effort_count,
                    } = match peek_result {
                        Ok(v) => v,
                        Err(e) => return Ok(Err(e)),
                    };

                    // effort: recursion effort
                    effort_count += child_effort_count;

                    elem_bucket_id
                } else {
                    None
                }
            }
        };
        if let Some(elem) = elem {
            return Ok(Ok(PeekResult {
                elem_bucket_id: Some(elem),
                effort_count,
            }));
        }
        current_remaining.set_empty(child_index);
    }
    Ok(Ok(PeekResult {
        elem_bucket_id: None,
        effort_count,
    }))
}

/// Resulting items and tentative ordering state from [`Network::peek`]
pub struct Peeked<'a, T> {
    items: Vec<&'a T>,
    source_buckets: Vec<BucketId>,
    root_order: Root,
    #[allow(dead_code, reason = "counter for tests")]
    effort_count: u64,
}
impl<'a, T> Peeked<'a, T> {
    /// Returns the peeked items
    #[must_use]
    pub fn items(&self) -> &[&'a T] {
        &self.items
    }
    /// Returns the source buckets for the peeked items
    #[must_use]
    pub fn source_buckets(&self) -> &[BucketId] {
        &self.source_buckets
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
    #[allow(dead_code, reason = "counter for tests")]
    /// For tests only, return the amount of effort required for this peek result
    pub(crate) fn get_effort_count(&self) -> u64 {
        self.effort_count
    }
}
/// Resulting tentative ordering state from [`Network::peek`] to apply in
/// [`Network::finalize_peeked`]
#[must_use]
pub struct PeekAccepted {
    new_root_order: Root,
}
