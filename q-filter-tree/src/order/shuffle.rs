// Copyright (C) 2021-2023  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use super::{Orderer, OrdererImpl, Weights};
use rand_chacha::ChaCha8Rng;
use std::ops::Range;

#[derive(Clone)]
/// Tracks the remaining (shuffled) indices to be returned
pub struct Shuffle {
    remaining_shuffled: Vec<usize>,
    rng: ChaCha8Rng,
}
impl Shuffle {
    fn new(rng: ChaCha8Rng, weights: &Weights) -> Self {
        let mut new = Self {
            remaining_shuffled: vec![],
            rng,
        };
        new.fill_remaining(weights);
        new
    }
    #[cfg(test)]
    fn from_seed(seed: u64, weights: &Weights) -> Self {
        use rand::SeedableRng;
        let rng = ChaCha8Rng::seed_from_u64(seed);
        Self::new(rng, weights)
    }
    fn fill_remaining(&mut self, weights: &Weights) {
        use rand::seq::SliceRandom;
        //TODO: consider changing behavior to persist "remaining",
        // only adding items to keep weight totals in check
        let mut idxs: Vec<_> = weights
            .iter()
            .map(|weight| usize::try_from(weight).expect("weight fit into usize"))
            .enumerate()
            .flat_map(|(idx, weight)| std::iter::repeat(idx).take(weight))
            .collect();
        idxs.shuffle(&mut self.rng);
        self.remaining_shuffled = idxs;
    }
    fn refill_if_empty(&mut self, weights: &Weights) {
        if self.remaining_shuffled.is_empty() {
            self.fill_remaining(weights);
        }
    }
}
impl From<&Weights> for Shuffle {
    fn from(weights: &Weights) -> Self {
        use rand::SeedableRng;
        let rng =
            ChaCha8Rng::from_rng(rand::thread_rng()).expect("thread_rng try_fill_bytes succeeds");
        Self::new(rng, weights)
    }
}
impl OrdererImpl for Shuffle {
    fn peek_unchecked(&self) -> Option<usize> {
        self.remaining_shuffled.last().copied()
    }
    fn validate(&self, index: usize, weights: &Weights) -> bool {
        weights.get(index).map_or(false, |weight| weight > 0)
    }
    fn advance(&mut self, weights: &Weights) {
        self.remaining_shuffled.pop();
        if self.remaining_shuffled.is_empty() {
            self.fill_remaining(weights);
        }
    }
}
impl Orderer for Shuffle {
    fn notify_removed(&mut self, removed: Range<usize>, weights: &Weights) {
        let removed_count = removed.clone().count();
        {
            // remove all occurrences of the `removed` index
            let mut search_index = 0;
            while let Some(value) = self.remaining_shuffled.get(search_index) {
                if removed.contains(value) || *value > (weights.len() + removed_count) {
                    self.remaining_shuffled.swap_remove(search_index);
                } else {
                    search_index += 1;
                }
            }
        }
        // decrement all occurrences above `removed` index
        for index in self
            .remaining_shuffled
            .iter_mut()
            .filter(|index| **index >= removed.end)
        {
            *index -= removed_count;
        }
        // refill if empty
        self.refill_if_empty(weights);
    }

    fn notify_changed(&mut self, changed: Option<usize>, weights: &Weights) {
        if let Some(changed) = changed {
            if let Some(weight) = weights.get(changed) {
                // find indices matching `changed`
                let mut match_indices: Vec<_> = self
                    .remaining_shuffled
                    .iter()
                    .copied()
                    .enumerate()
                    .filter(|(_, shuffle_elem)| *shuffle_elem == changed)
                    .map(|(index, _)| index)
                    .collect();
                // count number of matches
                let match_len: u32 = match_indices
                    .len()
                    .try_into()
                    .expect("shuffled weight indices should fit in a u32");
                // remove excess entries (from end)
                let excess = match_len
                    .checked_sub(weight)
                    .map(|excess| excess.try_into().expect("excess fits into usize"));
                if let Some(excess) = excess {
                    match_indices.sort_unstable();
                    for match_index in match_indices.iter().rev().take(excess) {
                        self.remaining_shuffled.swap_remove(*match_index);
                    }
                }
            } else {
                unreachable!("notify_changed on changed index out of bounds of weights");
            }
        } else {
            // easy, just rebuild
            self.remaining_shuffled.clear();
            self.fill_remaining(weights);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::order::tests::{resize_vec_to_len, Weights};
    use crate::order::State;

    use super::super::tests::{assert_peek_next, check_all, WeightVec};
    use super::super::Type;
    use super::Shuffle;

    #[test]
    fn all() {
        check_all(Type::Shuffle);
    }
    #[test]
    fn shuffles() {
        const SEED: u64 = 324_543_290;
        const SHUFFLE_10_TRUTH: &[&[usize; 10]; 5] = &[
            // depends on SEED, and RNG impl
            // (NOTE: INDEPENDENT of the Shuffle usage/impl, as verified below)
            &[5, 9, 8, 4, 3, 0, 6, 1, 2, 7],
            &[7, 8, 9, 5, 6, 4, 2, 3, 0, 1],
            &[5, 7, 1, 2, 3, 8, 9, 6, 4, 0],
            &[5, 1, 6, 2, 0, 7, 8, 9, 3, 4],
            &[3, 7, 6, 8, 1, 9, 0, 4, 2, 5],
        ];
        {
            // verify random determinism
            use rand::{seq::SliceRandom, SeedableRng};
            let mut rand_truth = rand_chacha::ChaCha8Rng::seed_from_u64(SEED);
            for rng_truth in SHUFFLE_10_TRUTH {
                let mut dummy_vec: Vec<_> = (0..10).collect();
                dummy_vec.shuffle(&mut rand_truth);
                dummy_vec.reverse();
                assert_eq!(&dummy_vec, rng_truth);
            }
        }
        let mut first = true;
        let mut weight_vec = WeightVec::new();
        for target_weights in &[[1, 2, 2, 5], [3, 1, 6, 0], [0, 0, 0, 10]] {
            let mut s = State::from(Shuffle::from_seed(SEED, &Weights::empty()));
            resize_vec_to_len(
                &mut weight_vec,
                &mut s,
                target_weights.len(),
                target_weights,
            );
            for rng_truth in SHUFFLE_10_TRUTH {
                let truth: Vec<_> = {
                    let ids: Vec<_> = weight_vec
                        .weights()
                        .iter()
                        .enumerate()
                        .flat_map(|(idx, count)| std::iter::repeat(idx).take(count as usize))
                        .collect();
                    if first {
                        assert_eq!(ids, vec![0, 1, 1, 2, 2, 3, 3, 3, 3, 3]);
                    }
                    rng_truth
                        .iter()
                        .copied()
                        .map(|remapped| ids.get(remapped).copied().expect("in range"))
                        .collect()
                };
                if first {
                    assert_eq!(truth, vec![3, 3, 3, 2, 2, 0, 3, 1, 1, 3]);
                }
                assert_eq!(
                    weight_vec.weights().iter().sum::<u32>() as usize,
                    truth.len(),
                    "for test to work"
                );
                for truth_elem in &truth {
                    assert_peek_next(&mut s, &weight_vec, Some(*truth_elem));
                }
                first = false;
            }
        }
    }
}
