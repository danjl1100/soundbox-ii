// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use crate::{Child, Network};

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
    ) -> Result<Vec<&'a T>, rand::Error> {
        let mut chosen = vec![];

        let child_nodes = &self.root;

        // TODO test for persisting child orders
        let mut child_orders: Vec<_> = std::iter::repeat_with(InOrder::default)
            .take(child_nodes.len())
            .collect();
        // TODO test for persisting traversal order
        let mut order = InOrder::default();
        let mut remaining = CountsRemaining::new(child_nodes.len());

        let mut debug_count = 0; // DEBUG TRAINING WHEELS
        while chosen.len() < peek_len {
            debug_count += 1; // DEBUG TRAINING WHEELS
            assert!(debug_count < 100); // DEBUG TRAINING WHEELS

            if remaining.is_empty() || child_nodes.is_empty() {
                return Ok(vec![]);
            }

            assert_eq!(child_nodes.len(), child_orders.len());

            let child_index = order
                .next_in(child_nodes)
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
                        let elem_index = child_order
                            .next_in(bucket_elems)
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
        Ok(chosen)
    }
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

#[derive(Default)]
struct InOrder(usize);
impl InOrder {
    fn next(&mut self, max_index: usize) -> usize {
        let next = if self.0 > max_index { 0 } else { self.0 };
        self.0 = next.wrapping_add(1);
        next
    }
    fn next_in<T>(&mut self, target: &[T]) -> Option<usize> {
        let max_index = target.len().checked_sub(1)?;
        let next = self.next(max_index);
        Some(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_order() {
        let mut uut = InOrder::default();
        assert_eq!(uut.next(5), 0);
        assert_eq!(uut.next(5), 1);
        assert_eq!(uut.next(5), 2);
        assert_eq!(uut.next(5), 3);
        assert_eq!(uut.next(5), 4);
        assert_eq!(uut.next(5), 5);
        //
        assert_eq!(uut.next(5), 0);
        //
        assert_eq!(uut.next(2), 1);
        assert_eq!(uut.next(2), 2);
        assert_eq!(uut.next(2), 0);
        assert_eq!(uut.next(2), 1);
        //
        assert_eq!(uut.next(1), 0);
        //
        assert_eq!(uut.next(0), 0);
        assert_eq!(uut.next(0), 0);
    }
}
