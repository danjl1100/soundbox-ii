use super::{Order, Weight};

/// Tracks count remaining for each element
pub struct State {
    weights: Vec<Weight>,
    count: Vec<Weight>,
    index: Option<usize>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            weights: vec![],
            count: vec![],
            index: None,
        }
    }
}
impl Order for State {
    fn get_weights(&self) -> &[Weight] {
        &self.weights
    }
    fn set_weights(&mut self, new_weights: &[Weight]) {
        /// ensures correct order of actions, packaging a struct of which actions are desired
        struct Act {
            clear_count: bool,
            do_advance: bool,
        }
        let old_len = self.weights.len();
        // pre-calculate from COUNT
        let was_restarting = // .
            self.count.iter().take(1).all(|&x| x == 1) && // .
            self.count.iter().skip(1).all(|&x| x == 0);
        // update weights
        self.weights = new_weights.to_vec();
        // verify current index is VALID
        let index_is_valid = self.index.map(|index| self.check_valid(index));
        // resize COUNT
        self.count.resize(new_weights.len(), 0);
        //
        let actions = match index_is_valid {
            Some(_) if was_restarting => {
                // dbg!("CONTINUE COUNT FROM INDEX = OLD_LEN");
                self.index.replace(old_len - 1);
                Act {
                    clear_count: true,
                    do_advance: true,
                }
            }
            Some(true) => {
                // continue on valid index
                // dbg!("CONTINUE, IT'S VALID :D");
                Act {
                    clear_count: false,
                    do_advance: false,
                }
            }
            Some(_) => {
                // dbg!("RESET ALL EXCEPT INDEX");
                Act {
                    clear_count: true,
                    do_advance: true,
                }
            }
            None => {
                // reset
                // dbg!("RESET ALL, INCLUDING INDEX");
                self.index = None;
                Act {
                    clear_count: true,
                    do_advance: true,
                }
            }
        };
        if actions.clear_count {
            self.count.fill(0);
        }
        if actions.do_advance {
            self.advance();
        }
    }
    fn advance(&mut self) {
        if self.weights.is_empty() || self.weights.iter().all(|x| *x == 0) {
            self.index = None;
        } else {
            let weights_len = self.weights.len();
            let mut mark_no_progress_since = None;
            self.simplify_count();
            loop {
                assert_eq!(
                    self.count.len(),
                    self.weights.len(),
                    "count length matches weights"
                );
                // increment
                let index = match self.index {
                    Some(prev_index) if prev_index + 1 < weights_len => prev_index + 1,
                    _ => {
                        // weights is NOT empty (per outer `else`) --> restart at index `0`
                        0
                    }
                };
                self.index.replace(index);
                // catch full-loop-no-progress
                match mark_no_progress_since {
                    Some(i) if i == index => {
                        mark_no_progress_since = None;
                        // reset
                        self.index = None;
                        self.count.fill(0);
                        continue;
                    }
                    _ => {}
                }
                // check count-remaining
                match (self.count.get_mut(index), self.weights.get(index)) {
                    (Some(count), Some(weight)) if *count >= *weight => {
                        // record "no progress" marker
                        mark_no_progress_since.get_or_insert(index);
                        continue;
                    }
                    (Some(count), Some(_)) => {
                        // found! increment count
                        *count += 1;
                        break;
                    }
                    (count_opt, weight_opt) => unreachable!(
                        "length mismatch at index {}: self.count_remaining {:?} to self.weights {:?}",
                        index, count_opt, weight_opt),
                }
            }
        }
    }
    fn peek_unchecked(&self) -> Option<usize> {
        self.index
    }
}
impl State {
    fn check_valid(&self, index: usize) -> bool {
        // check count-remaining
        match (self.count.get(index), self.weights.get(index)) {
            (Some(count), Some(weight)) if *count >= *weight => false,
            (Some(count), Some(weight)) if *count < *weight => true,
            _ => false,
        }
    }
    fn simplify_count(&mut self) -> bool {
        let simplify = self
            .count
            .iter()
            .zip(self.weights.iter())
            .all(|(count, weight)| count == weight);
        if simplify {
            self.count.fill(0);
        }
        simplify
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{assert_peek_next, check_all};
    use super::super::{State, Type, Weight};

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
        assert_eq!(check_counter, 1300); // rigging to ensure test does not get shorter while modifying
    }
    #[test]
    fn resizing() {
        let all_weights = &[1, 2, 2, 3, 0, 5];
        let test_sizes = (0..100).map(|i| (i % (all_weights.len() + 1)));
        let check_counter = do_run_round_robin(all_weights, test_sizes);
        assert_eq!(check_counter, 533); // rigging to ensure test does not get shorter while modifying
    }
    #[test]
    fn resizing_dynamic() {
        let all_weights = &[1, 2, 2, 3, 0, 5, 9, 0, 0, 3, 7];
        let double_len = all_weights.len() * 2;
        let test_sizes = (0..(double_len * 2)).map(|i| {
            let test_size = if i < double_len {
                i.min((double_len + 0) - i)
            } else {
                let i = i - double_len + 1;
                i.max((double_len + 1) - i) - all_weights.len()
            };
            test_size
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
        for test_size in test_sizes {
            let weights = &all_weights[0..test_size];
            dbg!(test_size, all_weights, weights, prev_index);
            let mut remaining = weights.to_vec();
            loop {
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
                        assert_peek_next(&mut s, weights, Some(index));
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
}
