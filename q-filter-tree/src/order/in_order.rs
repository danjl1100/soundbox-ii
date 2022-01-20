use super::{Orderer, Weight};

#[derive(Clone, Copy, Debug)]
struct Tally {
    index: usize,
    emitted: Weight,
    target: Weight,
}

/// Tracks weights and items remaining for current index
#[derive(Default, Debug)]
pub struct InOrder(Option<Tally>);
impl Orderer for InOrder {
    fn peek_unchecked(&self) -> Option<usize> {
        self.0.as_ref().map(|t| t.index)
    }
    fn advance(&mut self, weights: &[Weight]) {
        let filter_nonzero_weight = |(index, &target)| {
            if target > 0 {
                Some(Tally {
                    index,
                    emitted: 0,
                    target,
                })
            } else {
                None
            }
        };
        self.0 = self
            .0
            .and_then(|Tally { index, emitted, .. }| {
                let target = weights.get(index).copied().unwrap_or_default();
                let emitted = emitted + 1;
                if emitted < target {
                    Some(Tally {
                        index,
                        emitted,
                        target,
                    })
                } else {
                    let index = index + 1;
                    // search Tail then Head for first non-zero weight
                    let tail = weights.iter().enumerate().skip(index);
                    let head = weights.iter().enumerate();
                    tail.chain(head).find_map(filter_nonzero_weight)
                }
            })
            .or_else(|| {
                // find first index of non-zero weight
                weights.iter().enumerate().find_map(filter_nonzero_weight)
            });
    }
    fn notify_removed(&mut self, removed: usize, weights: &[Weight]) {
        match self.0 {
            Some(Tally { index, .. }) if index >= weights.len() => {
                // current is outside of range -> advance
                self.advance(weights);
            }
            Some(Tally { index, emitted, .. }) if removed < index => {
                // removed BEFORE current -> decrement index ONLY
                if let Some(index) = index.checked_sub(1) {
                    let target = weights[index];
                    self.0 = Some(Tally {
                        index,
                        emitted,
                        target,
                    });
                } else {
                    self.0 = None;
                    self.advance(weights);
                }
            }
            Some(Tally { index, .. }) if removed == index => {
                // removed AT current -> decrement then advance
                self.0 = index.checked_sub(1).map(|idx| Tally {
                    index,
                    emitted: 0,
                    target: weights[idx],
                });
                self.advance(weights);
            }
            Some(_) | None => {
                // removed AFTER current, or None -> no change
            }
        }
    }

    fn notify_changed(&mut self, changed: Option<usize>, weights: &[Weight]) {
        match (self.0, changed) {
            (_, None) => {
                // reinitialize (all changed)
                self.0 = None;
                self.advance(weights);
            }
            (Some(Tally { index, .. }), _) if index >= weights.len() => {
                // current is outside range -> advance
                self.advance(weights);
            }
            (Some(Tally { index, emitted, .. }), Some(changed)) if changed == index => {
                let new_target = weights[index];
                if emitted >= new_target {
                    self.advance(weights);
                }
            }
            (Some(_) | None, _) => {
                // nothing, no effect
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::order::tests::resize_vec_to_len;

    use super::super::tests::{assert_peek_next, check_all, to_weight_vec, WeightVec};
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
            for (index, &weight) in weight_vec.weights().iter().enumerate() {
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
        for i in 0..100 {
            let target_len = i % (all_weights.len() + 1);
            resize_vec_to_len(&mut weight_vec, &mut s, target_len, all_weights);
            let weight_vec = to_weight_vec(&all_weights[0..target_len]);
            for (index, &weight) in weight_vec.weights().iter().enumerate() {
                for _ in 0..weight {
                    assert_peek_next(&mut s, &weight_vec, Some(index));
                }
            }
        }
    }
}
