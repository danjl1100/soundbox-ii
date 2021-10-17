use super::{Order, Weight};

/// Tracks weights and items remaining for current index
pub struct State {
    weights: Vec<Weight>,
    index_remaining: Option<(usize, Weight)>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            weights: vec![],
            index_remaining: None,
        }
    }
}
impl Order for State {
    fn get_weights(&self) -> &[Weight] {
        &self.weights
    }
    fn set_weights(&mut self, new_weights: &[Weight]) {
        self.weights = new_weights.to_vec();
        self.index_remaining = None;
        self.advance();
    }
    fn advance(&mut self) {
        let filter_nonzero_weight = |(index, &weight)| {
            if weight > 0 {
                Some((index, weight - 1))
            } else {
                None
            }
        };
        self.index_remaining = self
            .index_remaining
            .and_then(|(index, weight)| {
                if weight > 0 {
                    Some((index, weight - 1))
                } else {
                    let index = index + 1;
                    // search Tail then Head for first non-zero weight
                    let tail = self.weights.iter().enumerate().skip(index);
                    let head = self.weights.iter().enumerate();
                    tail.chain(head).find_map(filter_nonzero_weight)
                }
            })
            .or_else(|| {
                // find first index of non-zero weight
                self.weights
                    .iter()
                    .enumerate()
                    .find_map(filter_nonzero_weight)
            });
    }
    fn peek_unchecked(&self) -> Option<usize> {
        // next index
        self.index_remaining.map(|(index, _)| index)
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{assert_peek_next, check_all};
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
        let weights = &[1, 2, 2, 3, 0, 5];
        let mut s = State::from(ty);
        for _ in 0..100 {
            for (index, &weight) in weights.iter().enumerate() {
                for _ in 0..weight {
                    assert_peek_next(&mut s, weights, Some(index));
                }
            }
        }
    }
    #[test]
    fn resizing() {
        let ty = Type::InOrder;
        //
        let all_weights = &[1, 2, 2, 3, 0, 5];
        let mut s = State::from(ty);
        for i in 0..100 {
            let weights = &all_weights[0..(i % (all_weights.len() + 1))];
            for (index, &weight) in weights.iter().enumerate() {
                for _ in 0..weight {
                    assert_peek_next(&mut s, weights, Some(index));
                }
            }
        }
    }
}
