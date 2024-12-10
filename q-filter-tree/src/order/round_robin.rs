// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Round Robin ordering continues cycling through all elements in order, skipping elements as
//! needed until each element visit-count is equal to its weight.

use super::{Orderer, OrdererImpl, Weights};
use std::{num::NonZeroUsize, ops::Range};

use index::Index;
mod index {
    use super::counts::Counts;
    use crate::weight_vec::Weights;
    use std::num::NonZeroUsize;

    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct Index(usize);
    impl Index {
        pub const ZERO: Self = Index(0);
        pub fn try_from_weights(weights: &Weights, counts: &Counts) -> Option<Self> {
            Self::try_from_weights_at(0, weights, counts)
        }
        pub fn try_from_weights_at(
            start: usize,
            weights: &Weights,
            counts: &Counts,
        ) -> Option<Self> {
            let tail = weights.iter().enumerate().zip(counts.iter()).skip(start);
            let head = weights.iter().enumerate().zip(counts.iter()).take(start);
            tail.chain(head)
                .find_map(|((index, weight), count)| (weight > count).then_some(Self(index)))
        }
        pub fn next(self, for_len: NonZeroUsize) -> Self {
            let next = self.inner() + 1;
            if next < for_len.into() {
                Self(next)
            } else {
                Self(0)
            }
        }
        pub fn try_adjust_removed_count(self, removed_count: usize) -> Option<Self> {
            self.inner().checked_sub(removed_count).map(Self)
        }
        pub fn inner(self) -> usize {
            self.0
        }
    }
}

use counts::Counts;
mod counts {
    use crate::{weight_vec::Weights, Weight};
    use std::ops::Range;

    #[derive(Clone, Debug, PartialEq)]
    pub struct Counts(Vec<Weight>);
    impl From<&Weights> for Counts {
        fn from(weights: &Weights) -> Self {
            Self(vec![0; weights.len()])
        }
    }
    impl Counts {
        pub fn simplify(&mut self, weights: &Weights) -> bool {
            // update length
            while self.0.len() < weights.len() {
                self.0.push(0);
            }
            // simplify
            let simplify = self
                .0
                .iter()
                .zip(weights.iter())
                .all(|(count, weight)| *count >= weight);
            if simplify {
                self.0.fill(0);
            }
            simplify
        }
        pub fn remove(&mut self, removed: &Range<usize>) {
            let old_len = self.len();
            let start = removed.start;
            let end = removed.end.min(old_len);
            for index in (start..end).rev() {
                self.0.remove(index);
            }
        }
        pub fn check_within_weight(&self, index: usize, weights: &Weights) -> bool {
            self.0
                .get(index)
                .zip(weights.get(index))
                .is_some_and(|(count, weight)| *count < weight)
        }
        pub fn len(&self) -> usize {
            self.0.len()
        }
        pub fn get_mut(&mut self, index: usize) -> Option<&mut Weight> {
            self.0.get_mut(index)
        }
        pub fn iter(&self) -> impl Iterator<Item = Weight> + '_ {
            self.0.iter().copied()
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
/// Tracks count remaining for each element
pub struct RoundRobin {
    counts: Counts,
    index: Option<Index>,
}
impl From<&Weights> for RoundRobin {
    fn from(weights: &Weights) -> Self {
        let counts = Counts::from(weights);
        let index = Index::try_from_weights(weights, &counts);
        RoundRobin { counts, index }
    }
}
impl RoundRobin {
    fn set_index_with<F>(&mut self, new_fn: F, weights: &Weights)
    where
        F: FnOnce(&Counts) -> Option<Index>,
    {
        if let Some(count) = self
            .index
            .map(Index::inner)
            .and_then(|index| self.counts.get_mut(index))
        {
            *count += 1;
            self.counts.simplify(weights);
        }
        self.index = new_fn(&self.counts);
    }
}
impl OrdererImpl for RoundRobin {
    fn peek_unchecked(&self) -> Option<usize> {
        self.index.map(Index::inner)
    }
    fn validate(&self, index: usize, weights: &Weights) -> bool {
        self.counts.check_within_weight(index, weights)
    }
    fn advance(&mut self, weights: &Weights) {
        match NonZeroUsize::try_from(weights.len()) {
            Err(_) => {
                self.set_index_with(|_| None, weights);
            }
            Ok(for_len) => {
                self.counts.simplify(weights);
                let mut mark_no_progress_since = None;
                let mut current_index = self.index.unwrap_or(Index::ZERO);
                self.set_index_with(
                    |counts| loop {
                        assert_eq!(counts.len(), weights.len(), "count length matches weights");
                        // increment
                        current_index = current_index.next(for_len);
                        let index = current_index.inner();
                        // catch full-loop-no-progress
                        match mark_no_progress_since {
                            Some(i) if i == index => {
                                break None;
                            }
                            _ => {}
                        }
                        // check count-remaining
                        if counts.check_within_weight(index, weights) {
                            break Some(current_index);
                        }
                        // record "no progress" marker
                        mark_no_progress_since.get_or_insert(index);
                    },
                    weights,
                );
            }
        }
        if let Some(index) = self.index.map(Index::inner) {
            assert!(self.validate(index, weights));
        }
    }
}
impl Orderer for RoundRobin {
    fn notify_removed(&mut self, removed: Range<usize>, weights: &Weights) {
        self.counts.remove(&removed);
        // adjust index (if needed)
        if let Some(old_index) = self.index {
            let index = old_index.inner();
            if removed.contains(&index) {
                // within range, stuff changed
                self.set_index_with(
                    |counts| Index::try_from_weights_at(removed.start, weights, counts),
                    weights,
                );
            } else if let Some(adjusted_index) = old_index.try_adjust_removed_count(removed.len()) {
                // after end, stuff changed
                self.set_index_with(
                    |counts| Index::try_from_weights_at(adjusted_index.inner(), weights, counts),
                    weights,
                );
            }
            // else: removed AFTER current --> no change
        }
    }
    fn notify_changed(&mut self, changed: Option<usize>, weights: &Weights) {
        if let Some(old_index) = self.index {
            let recalculate = match changed {
                Some(changed) if old_index.inner() == changed => true, // changed current, re-validate or search
                None => true,                                          // changed all, recalculate
                _ => false, // changed but not current, no change
            };
            if recalculate {
                self.set_index_with(
                    |counts| Index::try_from_weights_at(old_index.inner(), weights, counts),
                    weights,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RoundRobin;
    use crate::{
        order::{tests::CloneToNext, OrdererImpl},
        weight_vec::Weights,
    };

    impl CloneToNext for RoundRobin {
        fn next_for(&self, weights: &Weights) -> RoundRobin {
            let mut next = self.clone();
            next.advance(weights);
            next
        }
    }

    #[test]
    fn empty() {
        assert_chain! {
            let weights = vec![];
            let empty = RoundRobin::from(&weights);
            [
                start => None;
                let next => None;
            ]
            => {
                assert_eq!(empty, next); // advance is a no-op
                assert_eq!(empty, RoundRobin {
                    counts: (&Weights::from(vec![])).into(),
                    index: None,
                });
            }
        }
    }

    #[test]
    fn inactive_for_zeroed_weights() {
        for n in 1..10 {
            assert_chain! {
                let weights = vec![0; n];
                let inactive = RoundRobin::from(&weights);
                [
                    start => None;
                    let next => None;
                ]
                => assert_eq!(inactive, next) // advance is a no-op
            }
        }
    }

    #[test]
    fn single_active() {
        assert_chain! {
            let weights = vec![1];
            let single = RoundRobin::from(&weights);
            [
                start => Some(0);
                let next => Some(0);
            ]
            => {
                assert_eq!(single, next);
            }
        };
    }

    #[test]
    fn double_active() {
        assert_chain! {
            let weights = vec![1, 1];
            let double = RoundRobin::from(&weights);
            [
                start => Some(0);
                let next => Some(1);
                let next2 => Some(0);
                let next3 => Some(1);
            ]
            => {
                assert_eq!(double, next2);
                assert_eq!(next, next3);
            }
        };
    }

    #[test]
    fn triple_decreases() {
        assert_chain! {
            let weights = vec![1, 2, 1, 3];
            let triple = RoundRobin::from(&weights);
            [
                start => Some(0);
                let triple1 => Some(1);
                let triple2 => Some(2);
                let triple3 => Some(3);
                let triple4 => Some(1);
                let triple5 => Some(3);
                let triple6 => Some(3);
                let triple7 => Some(0);
            ]
            => {
                assert_eq!(triple, triple7);
            }
        }
    }
}

#[cfg(test)]
mod high_level_tests {
    use super::super::tests::{
        assert_peek_next, check_all, check_truncate, resize_vec_to_len, WeightVec,
    };
    use super::super::{State, Type};
    use crate::Weight;

    #[test]
    fn all() {
        let ty = Type::RoundRobin;
        check_all(ty);
    }
    #[test]
    fn longer() {
        let weights = &[1, 2, 2, 3, 0, 5];
        let test_sizes = (0..100).map(|_| weights.len());
        let check_counter = do_run_round_robin(weights, test_sizes);
        assert!(check_counter >= 1300, "{check_counter}"); // rigging to ensure test does not get shorter while modifying
    }
    #[test]
    fn varied_size() {
        let all_weights = &[1, 1];
        let test_sizes = [1, 2];
        let expected_count = 5;
        let check_counter = do_run_round_robin(all_weights, test_sizes);
        assert_eq!(check_counter, expected_count);
    }
    #[test]
    fn resizing() {
        let all_weights = &[1, 2, 2, 3, 0, 5];
        let test_sizes = (0..100).map(|i| (i % (all_weights.len() + 1)));
        let check_counter = do_run_round_robin(all_weights, test_sizes);
        assert!(check_counter >= 533, "{check_counter:?}"); // rigging to ensure test does not get shorter while modifying
    }
    #[test]
    #[ignore] // TODO figure out why checked-number 194 and below is bad (214/706 fail counts)
    fn resizing_dynamic() {
        let all_weights = &[1, 2, 2, 3, 0, 5, 9, 0, 0, 3, 7];
        let double_len = all_weights.len() * 2;
        let test_sizes = (0..(double_len * 2)).map(|i| {
            // test size
            if i < double_len {
                i.min(double_len - i)
            } else {
                let i = i - double_len + 1;
                i.max((double_len + 1) - i) - all_weights.len()
            }
        });
        let check_counter = do_run_round_robin(all_weights, test_sizes);
        assert_eq!(check_counter, 612); // rigging to ensure test does not get shorter while modifying
    }
    fn do_run_round_robin<I>(all_weights: &[Weight], test_sizes: I) -> usize
    where
        I: IntoIterator<Item = usize>,
    {
        let ty = Type::RoundRobin;
        //
        let mut s = State::from(ty);
        let mut prev_index = None;
        let mut check_counter = 0;
        let mut weight_vec = WeightVec::new();
        let mut remaining = vec![];
        for test_size in test_sizes {
            resize_vec_to_len(&mut weight_vec, &mut s, test_size, all_weights);
            //
            while let Some(next) = weight_vec.weights().get(remaining.len()) {
                remaining.push(next);
            }
            remaining.truncate(weight_vec.weights().len());
            //
            let mut can_refill = 1;
            loop {
                if can_refill > 0 && remaining.iter().all(|x| *x == 0) {
                    can_refill -= 1;
                    remaining = weight_vec.weights().iter().collect();
                }
                //
                let mut popped = false;
                let start_index = match prev_index {
                    Some(prev_index) if prev_index < remaining.len() => prev_index + 1,
                    Some(_) | None => 0,
                };
                let (front, tail) = remaining.split_at_mut(start_index);
                let front_iter = front.iter_mut().enumerate();
                let tail_iter = tail
                    .iter_mut()
                    .enumerate()
                    .map(|(idx, val)| (idx + start_index, val));
                for (index, remaining) in tail_iter.chain(front_iter) {
                    if *remaining > 0 {
                        assert_peek_next(&mut s, &weight_vec, Some(index));
                        //
                        popped = true;
                        *remaining -= 1;
                        //
                        prev_index.replace(index);
                        check_counter += 1;
                    }
                }
                if !popped {
                    break;
                }
            }
        }
        check_counter
    }
    #[test]
    fn truncate() {
        let ty = Type::RoundRobin;
        check_truncate(ty);
    }
}
