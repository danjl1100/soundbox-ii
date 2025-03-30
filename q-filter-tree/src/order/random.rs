// Copyright (C) 2021-2025  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use super::{Orderer, OrdererImpl, Weights};
use rand_chacha::ChaCha8Rng;
use std::ops::Range;

#[derive(Clone)]
/// Random number generator and current index.
pub struct Random {
    current: Option<usize>,
    rng: ChaCha8Rng,
}
impl Random {
    fn new(rng: ChaCha8Rng, weights: &Weights) -> Self {
        let mut new = Self { current: None, rng };
        new.advance(weights);
        new
    }
    #[cfg(test)]
    fn from_seed(seed: u64, weights: &Weights) -> Self {
        use rand::SeedableRng;
        let rng = ChaCha8Rng::seed_from_u64(seed);
        Self::new(rng, weights)
    }
}
impl From<&Weights> for Random {
    fn from(weights: &Weights) -> Self {
        use rand::SeedableRng;
        let rng =
            ChaCha8Rng::from_rng(rand::thread_rng()).expect("thread_rng try_fill_bytes succeeds");
        Self::new(rng, weights)
    }
}
impl OrdererImpl for Random {
    fn peek_unchecked(&self) -> Option<usize> {
        self.current
    }

    fn validate(&self, index: usize, weights: &Weights) -> bool {
        index < weights.len()
    }

    fn advance(&mut self, weights: &Weights) {
        use rand::distributions::WeightedIndex;
        use rand::prelude::Distribution;

        self.current = WeightedIndex::new(weights)
            .ok()
            .map(|dist| dist.sample(&mut self.rng));
    }
}
impl Orderer for Random {
    fn notify_removed(&mut self, removed: Range<usize>, weights: &Weights) {
        match self.current {
            Some(current) if removed.contains(&current) || current >= weights.len() => {
                self.advance(weights);
            }
            _ => {}
        }
    }

    fn notify_changed(&mut self, changed: Option<usize>, weights: &Weights) {
        if let Some(current) = self.current {
            match (changed, weights.get(current)) {
                (Some(changed), Some(new_weight))
                    if (current == changed && new_weight == 0) || current >= weights.len() =>
                {
                    self.advance(weights);
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::check_all;
    use super::Random;
    use crate::order::{tests::WeightVec, State, Type};

    #[test]
    fn all() {
        check_all(Type::Random);
    }

    //NOTE: The `Random` behavior is less predictable than `Shuffle`
    // (not neatly packed in separate rounds), and so the tests below are somewhat arbitrary.
    // The results are locked in by a seed input, but the precision of the test is dubious at best.
    // These tests can allow false-passes for some diabolical implementations.
    //

    #[test]
    fn verify_masked() {
        const MAX_INDEX: usize = 5;
        for zero_index in 0..MAX_INDEX {
            const SEED: u64 = 243_597_435;
            let weights = {
                let mut weights = vec![1; MAX_INDEX];
                weights[zero_index] = 0;
                weights.into()
            };
            let mut s = State::from(Random::from_seed(SEED, &weights));
            let mut counts = vec![0; weights.len()];
            for _ in 0..100 {
                let value = s.peek(&weights).expect("always some index");
                counts[value] += 1;
                s.next(&weights);
            }
            dbg!(&counts);
            assert_eq!(counts.remove(zero_index), 0, "zero index");
            assert!(
                counts.iter().all(|x| *x > 0),
                "all others nonzero {counts:?}"
            );
        }
    }

    #[test]
    fn verify_change_all_zero_yields_none() {
        const SEED: u64 = 243_597_435;
        const N: usize = 100;
        let mut weight_vec = std::iter::repeat_n((1, ()), 50).collect::<WeightVec<_>>();
        let weights = weight_vec.weights();
        let mut s = State::from(Random::from_seed(SEED, weights));
        for _ in 0..N {
            assert!(s.peek(weights).is_some());
            assert!(s.next(weights).is_some());
        }
        let current = s.peek(weights).expect("before zero, some index");
        // set current to zero-weight
        let old_value_result = weight_vec.ref_mut(&mut s).set_weight(current, 0);
        assert_eq!(old_value_result, Ok(1));
        //
        let weights = weight_vec.weights();
        assert_ne!(s.peek(weights), Some(current));
        // set ALL to zero-weight
        let mut weight_vec_mut = weight_vec.ref_mut(&mut s);
        for i in 0..weight_vec_mut.len() {
            weight_vec_mut.set_weight(i, 0).expect("valid index");
        }
        //
        let weights = weight_vec.weights();
        for _ in 0..N {
            assert!(s.peek(weights).is_none());
            assert!(s.next(weights).is_none());
        }
    }

    #[test]
    fn verify_removed_changes_current() {
        const SEED: u64 = 651_874_963;
        const N: usize = 100;
        let mut weight_vec = std::iter::repeat_n((1, ()), 5).collect::<WeightVec<_>>();
        let weights = weight_vec.weights();
        let mut s = State::from(Random::from_seed(SEED, weights));
        // sees all items
        let mut waiting_to_see = vec![Some(()); weights.len()];
        for _ in 0..N {
            let index = s.next(weights).expect("items remaining");
            waiting_to_see[index].take();
        }
        assert!(waiting_to_see.iter().all(Option::is_none));
        // remove items
        let mut iter_count = 0;
        let mut differences_count = 0;
        while let Some(index) = s.next(weight_vec.weights()) {
            assert_eq!(Some(index), s.peek(weight_vec.weights()));
            weight_vec
                .ref_mut(&mut s)
                .remove(index)
                .expect("yielded index exists");
            if matches!(s.peek(weight_vec.weights()), Some(new_index) if new_index != index) {
                differences_count += 1;
            }
            iter_count += 1;
            assert!(iter_count < 100_000, "guard against infinite loop");
        }
        assert!(differences_count + 2 > weight_vec.len()); // arbitrary, depends on seed
        assert!(weight_vec.weights().is_empty());
    }

    #[test]
    fn verify_changed_to_zero_changes_current() {
        const SEED: u64 = 651_874_963;
        let mut weight_vec = std::iter::repeat_n((1, ()), 5).collect::<WeightVec<_>>();
        let weights = weight_vec.weights();
        let mut s = State::from(Random::from_seed(SEED, weights));
        //
        let mut iter_count = 0;
        let mut differences_count = 0;
        // remove items
        while let Some(index) = s.next(weight_vec.weights()) {
            assert_eq!(Some(index), s.peek(weight_vec.weights()));
            // set weight to 0
            weight_vec
                .ref_mut(&mut s)
                .set_weight(index, 0)
                .expect("yielded index exists");
            if matches!(s.peek(weight_vec.weights()), Some(new_index) if new_index != index) {
                differences_count += 1;
            }
            iter_count += 1;
            assert!(iter_count < 100_000, "guard against infinite loop");
        }
        assert!(differences_count + 2 > weight_vec.len()); // arbitrary, depends on seed
    }
}
