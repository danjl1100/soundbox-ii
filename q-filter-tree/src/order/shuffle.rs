use super::{Order, Weight};
use rand;
use rand_chacha::ChaCha8Rng;

/// Tracks the remaining (shuffled) indices to be returned
pub struct State {
    weights: Vec<Weight>,
    remaining_shuffled: Vec<usize>,
    rng: ChaCha8Rng,
}
impl State {
    fn new(rng: ChaCha8Rng) -> Self {
        Self {
            weights: vec![],
            remaining_shuffled: vec![],
            rng,
        }
    }
    #[cfg(test)]
    fn from_seed(seed: u64) -> Self {
        use rand::SeedableRng;
        let rng = ChaCha8Rng::seed_from_u64(seed);
        Self::new(rng)
    }
    fn fill_remaining(&mut self) {
        use rand::seq::SliceRandom;
        use std::convert::TryFrom;
        //TODO: consider changing behavior to persist "remaining",
        // only adding items to keep weight totals in check
        let mut idxs: Vec<_> = self
            .weights
            .iter()
            .map(|&weight| usize::try_from(weight).expect("weight fit into usize"))
            .enumerate()
            .flat_map(|(idx, weight)| std::iter::repeat(idx).take(weight))
            .collect();
        idxs.shuffle(&mut self.rng);
        self.remaining_shuffled = idxs;
    }
}
impl Default for State {
    fn default() -> Self {
        use rand::SeedableRng;
        let rng =
            ChaCha8Rng::from_rng(rand::thread_rng()).expect("thread_rng try_fill_bytes succeeds");
        Self::new(rng)
    }
}
impl Order for State {
    fn get_weights(&self) -> &[Weight] {
        &self.weights
    }
    fn peek_unchecked(&self) -> Option<usize> {
        self.remaining_shuffled.last().copied()
    }
    fn set_weights(&mut self, new_weights: &[Weight]) {
        self.weights = new_weights.to_vec();
        self.fill_remaining();
    }
    fn advance(&mut self) {
        self.remaining_shuffled.pop();
        if self.remaining_shuffled.is_empty() {
            self.fill_remaining();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{assert_peek_next, check_all};
    use super::super::{State, Type};
    use super::State as ShuffleState;

    #[test]
    fn all() {
        check_all(Type::Shuffle);
    }
    #[test]
    fn shuffles() {
        const SEED: u64 = 324543290;
        const SHUFFLE_10_TRUTH: &[&[usize; 10]; 5] = &[
            // depends on SEED, and RNG impl
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
        for weights in &[[1, 2, 2, 5], [3, 1, 6, 0], [0, 0, 0, 10]] {
            let mut s = State::Shuffle(ShuffleState::from_seed(SEED));
            for rng_truth in SHUFFLE_10_TRUTH {
                let truth: Vec<_> = {
                    let ids: Vec<_> = weights
                        .iter()
                        .enumerate()
                        .flat_map(|(idx, count)| std::iter::repeat(idx).take(*count as usize))
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
                    weights.iter().copied().sum::<u32>() as usize,
                    truth.len(),
                    "for test to work"
                );
                for truth_elem in &truth {
                    assert_peek_next(&mut s, weights, Some(*truth_elem));
                }
                first = false;
            }
        }
    }
}
