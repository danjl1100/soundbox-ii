use rand_chacha::ChaCha8Rng;

use crate::Weight;

use super::Orderer;

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

    fn advance(&mut self, weights: &[Weight]) {
        use rand::distributions::WeightedIndex;
        use rand::prelude::Distribution;

        self.current = WeightedIndex::new(weights)
            .ok()
            .map(|dist| dist.sample(&mut self.rng));
    }

    fn notify_removed(&mut self, removed: usize, weights: &[Weight]) {
        match self.current {
            Some(current) if current == removed || current >= weights.len() => {
                self.advance(weights);
            }
            _ => {}
        }
    }

    fn notify_changed(&mut self, changed: Option<usize>, weights: &[Weight]) {
        if let Some(current) = self.current {
            match (changed, weights.get(current)) {
                (Some(changed), Some(&new_weight))
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
