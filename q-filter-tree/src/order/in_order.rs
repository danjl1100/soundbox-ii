// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use super::{Orderer, OrdererImpl};
use crate::weight_vec::Weights;
use std::{num::NonZeroUsize, ops::Range};

use tally::Tally;
mod tally {
    use crate::{weight_vec::Weights, Weight};
    use std::num::NonZeroUsize;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct Tally {
        index: usize,
        emitted: Weight,
    }
    impl Tally {
        /// Construct at the specified index if the weight is non-zero
        fn try_from_nonzero((index, weight): (usize, Weight)) -> Option<Self> {
            (weight > 0).then_some(Self { index, emitted: 0 })
        }
        /// Attempt to find first index of non-zero weight
        pub fn try_from_weights(weights: &Weights) -> Option<Self> {
            weights.iter().enumerate().find_map(Self::try_from_nonzero)
        }
        /// Search for the first non-zero weight, starting at the specified index and wrapping
        pub fn try_from_weights_start_at(start: usize, weights: &Weights) -> Option<Self> {
            // search Tail then Head for first non-zero weight
            let tail = weights.iter().enumerate().skip(start);
            let head = weights.iter().enumerate().take(start);
            tail.chain(head).find_map(Self::try_from_nonzero)
        }
        /// Increment the `emitted` count and advances to the next index if needed
        pub fn next(mut self, weights: &Weights) -> Option<Self> {
            self.emitted += 1;

            if self.validate(weights) {
                Some(self)
            } else {
                Self::try_from_weights_start_at(self.index + 1, weights)
            }
        }
        /// Returns the current index (if valid) otherwise finds the next valid index
        pub fn validate_or_else_next(self, weights: &Weights) -> Option<Self> {
            if self.validate(weights) {
                Some(self)
            } else {
                Self::try_from_weights_start_at(self.index + 1, weights)
            }
        }
        /// Modify the index by subtracting the specified count
        pub fn try_adjust_removed_count(self, count: NonZeroUsize) -> Option<Self> {
            self.index
                .checked_sub(count.into())
                .map(|index| Self { index, ..self })
        }
        /// Return `true` if the current emitted count is within the specified Weights
        pub fn validate(self, weights: &Weights) -> bool {
            weights
                .get(self.index)
                .is_some_and(|weight| self.emitted < weight)
        }
        pub const fn index(self) -> usize {
            self.index
        }
    }
}

/// Tracks weights and items remaining for current index
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InOrder(Option<Tally>);
impl From<&Weights> for InOrder {
    fn from(weights: &Weights) -> Self {
        Self(Tally::try_from_weights(weights))
    }
}
impl OrdererImpl for InOrder {
    fn peek_unchecked(&self) -> Option<usize> {
        self.0.as_ref().map(|tally| tally.index())
    }
    fn validate(&self, index: usize, weights: &Weights) -> bool {
        self.0
            .is_some_and(|tally| index == tally.index() && tally.validate(weights))
    }
    fn advance(&mut self, weights: &Weights) {
        self.0 = self
            .0
            .and_then(|tally| tally.next(weights))
            .or_else(|| Tally::try_from_weights(weights));
    }
}
impl Orderer for InOrder {
    fn notify_removed(&mut self, removed: Range<usize>, weights: &Weights) {
        if let Some(tally) = self.0 {
            if removed.contains(&tally.index()) {
                // removed AT current --> start at non-removed element (which slid down)
                self.0 = Tally::try_from_weights_start_at(removed.start, weights);
            } else if let Some(adjusted_index) = NonZeroUsize::try_from(removed.count())
                .ok()
                .and_then(|removed_count| tally.try_adjust_removed_count(removed_count))
            {
                // successfuly moved tally down (slid down) e.g. removed BEFORE current
                // --> use validated (or next) index
                self.0 = adjusted_index.validate_or_else_next(weights);
            }
            // else: removed AFTER current --> no change
        }
        // else: no tally present to update --> no change
    }

    fn notify_changed(&mut self, changed: Option<usize>, weights: &Weights) {
        match (self.0, changed) {
            (_, None) => {
                // reinitialize (all changed)
                self.0 = Tally::try_from_weights(weights);
            }
            (Some(tally), _) if tally.index() >= weights.len() => {
                // current is outside range -> reinitialize
                self.0 = Tally::try_from_weights(weights);
            }
            (Some(tally), Some(changed)) if changed == tally.index() => {
                self.0 = tally.validate_or_else_next(weights);
            }
            (Some(_) | None, _) => {
                // nothing, no effect
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::InOrder;
    use crate::{
        order::{tests::CloneToNext, Orderer, OrdererImpl},
        weight_vec::Weights,
    };

    impl CloneToNext for InOrder {
        fn next_for(&self, weights: &Weights) -> Self {
            let mut next = self.clone();
            next.advance(weights);
            next
        }
    }

    #[test]
    fn empty() {
        assert_chain! {
            let weights = vec![];
            let start = InOrder::from(&weights);
            [
                start => None;
                let next => None;
            ]
            => assert_eq!(start, next)
        }
    }

    #[test]
    fn zero() {
        assert_chain! {
            let weights = vec![0];
            let start = InOrder::from(&weights);
            [
                start => None;
                let next => None;
            ]
            => assert_eq!(start, next)
        }
    }

    #[test]
    fn one() {
        assert_chain! {
            let weights = vec![1];
            let start = InOrder::from(&weights);
            [
                start => Some(0);
                let next => Some(0);
            ]
            => assert_eq!(start, next)
        }
    }

    #[test]
    fn two() {
        assert_chain! {
            let weights = vec![1, 1];
            let start = InOrder::from(&weights);
            [
                start => Some(0);
                let next1 => Some(1);
                let next2 => Some(0);
            ]
            => assert_eq!(start, next2)
        }
    }

    #[test]
    fn two_to_one() {
        let start = assert_chain! {
            let weights = vec![1, 1];
            let start = InOrder::from(&weights);
            [
                start => Some(0);
                let next1 => Some(1);
                let next2 => Some(0);
            ]
            => {
                assert_eq!(start, next2);
                start
            }
        };
        assert_chain! {
            let weights = vec![1];
            let start = start;
            [
                start => Some(0);
                let short1 => Some(0);
                let short2 => Some(0);
            ]
            => assert_eq!(short1, short2)
        }
    }

    #[test]
    fn three_to_one() {
        assert_chain! {
            let weights = vec![1, 1, 1];
            let three0 = InOrder::from(&weights);
            [
                start => Some(0);
                let three1 => Some(1);
                let three2 => Some(2);
                let three3 => Some(0);
            ]
            weights = vec![1, 2];
            start = three0;
            [
                start => Some(0);
                let two1 => Some(1);
                let two2 => Some(1);
                let two3 => Some(0);
            ]
            weights = vec![1];
            start = three1;
            [
                start => Some(1);
                let three1_next1 => Some(0);
            ]
            start = three2;
            [
                start => Some(2);
                let three2_next1 => Some(0);
            ]
            start = three3;
            [
                start => Some(0);
                let three3_next1 => Some(0);
            ]
            weights = vec![0];
            start = three1;
            [
                start => Some(1);
                let three1_next0 => None;
            ]
            start = three2;
            [
                start => Some(2);
                let three2_next0 => None;
            ]
            start = three3;
            [
                start => Some(0);
                let three3_next0 => None;
            ]
            => {
                assert_eq!(three0, three3);
                let original = three0;
                //
                assert_ne!(two1, original);
                assert_ne!(two2, original);
                assert_eq!(two3, original);
                //
                assert_eq!(three1_next1, original);
                assert_eq!(three2_next1, original);
                assert_eq!(three3_next1, original);
                //
                assert_ne!(three1_next0, original);
                assert_eq!(three1_next0, three2_next0);
                assert_eq!(three1_next0, three3_next0);
            }
        }
    }
    #[test]
    #[rustfmt::skip]
    fn seven_remove_mid() {
        struct Named { seven1: InOrder, seven5: InOrder, seven6: InOrder, seven7: InOrder, seven8: InOrder }
        let Named { mut seven1, mut seven5, mut seven6, mut seven7, mut seven8 } = assert_chain! {
            let weights = vec![1, 1, 2, 1, 2, 1, 1];
            let seven0 = InOrder::from(&weights);
            [
                start => Some(0);
                let seven1 => Some(1);
                let seven2 => Some(2);
                let seven3 => Some(2);
                let seven4 => Some(3);
                let seven5 => Some(4);
                let seven6 => Some(4);
                let seven7 => Some(5);
                let seven8 => Some(6);
                let seven9 => Some(0);
            ]
            => {
                assert_eq!(seven0, seven9);
                Named { seven1, seven5, seven6, seven7, seven8 }
            }
        };
        let weights = Weights::from(vec![1, 1, 1, 1]);
        // before mid
        seven1.notify_removed(2..5, &weights);
        assert_chain! {
            let weights = weights.clone();
            let four0 = seven1;
            [
                start => Some(1); // identical to before removal
                let four1 => Some(2);
                let four2 => Some(3);
                let four3 => Some(0);
                let four4 => Some(1);
            ]
            => assert_eq!(four0, four4)
        }
        // at mid
        seven5.notify_removed(2..5, &weights);
        assert_chain! {
            let weights = weights.clone();
            let four0 = seven5;
            [
                start => Some(2); // removed (was 4) so next is at the removal point 2
                let four1 => Some(3);
                let four2 => Some(0);
                let four3 => Some(1);
                let four4 => Some(2);
            ]
            => assert_eq!(four0, four4)
        }
        // at mid
        seven6.notify_removed(2..5, &weights);
        assert_chain! {
            let weights = weights.clone();
            let four0 = seven6;
            [
                start => Some(2); // removed (was 4) so next is at the removal point 2
                let four1 => Some(3);
                let four2 => Some(0);
                let four3 => Some(1);
                let four4 => Some(2);
            ]
            => assert_eq!(four0, four4)
        }
        // after mid
        seven7.notify_removed(2..5, &weights);
        assert_chain! {
            let weights = weights.clone();
            let four0 = seven7;
            [
                start => Some(2); // shifted from 5 (down 3) after removal to 2
                let four1 => Some(3);
                let four2 => Some(0);
                let four3 => Some(1);
                let four4 => Some(2);
            ]
            => assert_eq!(four0, four4)
        }
        // after mid
        seven8.notify_removed(2..5, &weights);
        assert_chain! {
            let weights = weights.clone();
            let four0 = seven8;
            [
                start => Some(3); // shifted from 6 (down 3) after removal to 3
                let four1 => Some(0);
                let four2 => Some(1);
                let four3 => Some(2);
                let four4 => Some(3);
            ]
            => assert_eq!(four0, four4)
        }
    }
}

#[cfg(test)]
mod higher_level_tests {
    use super::super::tests::{
        assert_peek_next, check_all, check_truncate, resize_vec_to_len, to_weight_vec, WeightVec,
    };
    use super::super::{State, Type};

    #[test]
    fn all() {
        let ty = Type::InOrder;
        check_all(ty);
    }
    #[test]
    fn longer() {
        let ty = Type::InOrder;
        //
        let weight_vec = to_weight_vec(&[1, 2, 2, 3, 0, 5]);
        let mut s = State::from(ty);
        for _ in 0..100 {
            for (index, weight) in weight_vec.weights().iter().enumerate() {
                for _ in 0..weight {
                    assert_peek_next(&mut s, &weight_vec, Some(index));
                }
            }
        }
    }
    #[test]
    fn resizing() {
        let ty = Type::InOrder;
        //
        let all_weights = &[1, 2, 2, 3, 0, 5];
        let mut weight_vec = WeightVec::new();
        let mut s = State::from(ty);
        let mut prev_len = None;
        for i in 0..100 {
            let target_len = i % (all_weights.len() + 1);
            resize_vec_to_len(&mut weight_vec, &mut s, target_len, all_weights);
            let weight_vec = to_weight_vec(&all_weights[0..target_len]);
            //
            let tail_len = prev_len.unwrap_or_default();
            let tail = weight_vec.weights().iter().enumerate().skip(tail_len);
            let whole = weight_vec.weights().iter().enumerate();
            //
            for (index, weight) in tail.chain(whole) {
                for _ in 0..weight {
                    assert_peek_next(&mut s, &weight_vec, Some(index));
                }
            }
            prev_len.replace(target_len);
        }
    }
    #[test]
    fn truncate() {
        let ty = Type::InOrder;
        check_truncate(ty);
    }
}
