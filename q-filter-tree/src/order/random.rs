// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use super::{Orderer, Weights};
use rand_chacha::ChaCha8Rng;
use std::ops::Range;

#[derive(Clone)]
/// Random number generator and current index.
pub struct Random {
    current: Option<usize>,
    rng: ChaCha8Rng,
}
impl Random {
    fn new(rng: ChaCha8Rng) -> Self {
        Self { current: None, rng }
    }
    #[cfg(test)]
    fn from_seed(seed: u64) -> Self {
        use rand::SeedableRng;
        let rng = ChaCha8Rng::seed_from_u64(seed);
        Self::new(rng)
    }
}
impl Default for Random {
    fn default() -> Self {
        use rand::SeedableRng;
        let rng =
            ChaCha8Rng::from_rng(rand::thread_rng()).expect("thread_rng try_fill_bytes succeeds");
        Self::new(rng)
    }
}
impl Orderer for Random {
    fn peek_unchecked(&self) -> Option<usize> {
        self.current
    }

    fn advance(&mut self, weights: &Weights) {
        use rand::distributions::WeightedIndex;
        use rand::prelude::Distribution;

        self.current = WeightedIndex::new(weights)
            .ok()
            .map(|dist| dist.sample(&mut self.rng));
    }

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
    use super::super::Type;

    #[test]
    fn all() {
        check_all(Type::Random);
    }

    //TODO add more tests ...?
    // verify that "weight masks" will likely-never show invalid values
}
