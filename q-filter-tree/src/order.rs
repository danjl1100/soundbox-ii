//! Order of picking nodes from children nodes, given the node [`Weight`]s.
//!
//! # Examples:
//!
//! 1. [`Type::InOrder`]
//!
//! Visits child nodes **in order**.  Weights `[2, 1, 3]` will yield `AABCCC AABCCC ...`
//! ```
//! use q_filter_tree::{Tree, error::PopError, OrderType};
//! let mut t: Tree<_, ()> = Tree::default();
//! let root = t.root_id();
//! let mut root_ref = root.try_ref(&mut t).unwrap();
//! //
//! root_ref.set_order(OrderType::InOrder);
//! //
//! let childA = root_ref.add_child(Some(2));
//! let childB = root_ref.add_child(Some(1));
//! let childC = root_ref.add_child(Some(3));
//! let mut childA_ref = childA.try_ref(&mut t).unwrap();
//! childA_ref.push_item("A1");
//! childA_ref.push_item("A2");
//! let mut childB_ref = childB.try_ref(&mut t).unwrap();
//! childB_ref.push_item("B1");
//! let mut childC_ref = childC.try_ref(&mut t).unwrap();
//! childC_ref.push_item("C1");
//! childC_ref.push_item("C2");
//! childC_ref.push_item("C3");
//! //
//! let mut root_ref = root.try_ref(&mut t).unwrap();
//! assert_eq!(root_ref.pop_item(), Ok("A1"));
//! assert_eq!(root_ref.pop_item(), Ok("A2"));
//! assert_eq!(root_ref.pop_item(), Ok("B1"));
//! assert_eq!(root_ref.pop_item(), Ok("C1"));
//! assert_eq!(root_ref.pop_item(), Ok("C2"));
//! assert_eq!(root_ref.pop_item(), Ok("C3"));
//! assert_eq!(root_ref.pop_item(), Err(PopError::Empty(root.into())));
//! ```
//!
//! 2. [`Type::RoundRobin`]
//!
//! Cycles through child nodes sequentially, picking one item until reaching each child's `Weight`.  Weights `[2, 1, 3]` will yield `ABCACC ABCACC...`
//! ```
//! use q_filter_tree::{Tree, error::PopError, OrderType};
//! let mut t: Tree<_, ()> = Tree::default();
//! let root = t.root_id();
//! let mut root_ref = root.try_ref(&mut t).unwrap();
//! //
//! root_ref.set_order(OrderType::RoundRobin);
//! //
//! let childA = root_ref.add_child(Some(2));
//! let childB = root_ref.add_child(Some(1));
//! let childC = root_ref.add_child(Some(3));
//! let mut childA_ref = childA.try_ref(&mut t).unwrap();
//! childA_ref.push_item("A1");
//! childA_ref.push_item("A2");
//! let mut childB_ref = childB.try_ref(&mut t).unwrap();
//! childB_ref.push_item("B1");
//! let mut childC_ref = childC.try_ref(&mut t).unwrap();
//! childC_ref.push_item("C1");
//! childC_ref.push_item("C2");
//! childC_ref.push_item("C3");
//! //
//! let mut root_ref = root.try_ref(&mut t).unwrap();
//! assert_eq!(root_ref.pop_item(), Ok("A1"));
//! assert_eq!(root_ref.pop_item(), Ok("B1"));
//! assert_eq!(root_ref.pop_item(), Ok("C1"));
//! assert_eq!(root_ref.pop_item(), Ok("A2"));
//! assert_eq!(root_ref.pop_item(), Ok("C2"));
//! assert_eq!(root_ref.pop_item(), Ok("C3"));
//! assert_eq!(root_ref.pop_item(), Err(PopError::Empty(root.into())));
//! ```

use super::Weight;
use serde::{Deserialize, Serialize};

/// Method of determining Order
#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Eq, PartialEq, Clone, Copy, Serialize, Deserialize)]
pub enum Type {
    /// Picks [`Weight`] items from one node before moving to the next node
    InOrder,
    /// Picks items from each node in turn, up to maximum of [`Weight`] items per cycle.
    RoundRobin,
    // TODO
    // /// Shuffles the order of items given by [`Self::InOrder`] for each cycle.
    // Shuffle,
    // /// Randomly selects items based on the relative [`Weight`]s.
    // Random,
}

#[allow(missing_docs)]
/// State for tracking Ordering progression
pub enum State {
    InOrder(InOrderState),
    RoundRobin(RoundRobinState),
}
impl From<Type> for State {
    fn from(ty: Type) -> Self {
        match ty {
            Type::InOrder => Self::InOrder(InOrderState::default()),
            Type::RoundRobin => Self::RoundRobin(RoundRobinState::default()),
        }
    }
}
impl From<&State> for Type {
    fn from(state: &State) -> Self {
        match state {
            State::InOrder(_) => Self::InOrder,
            State::RoundRobin(_) => Self::RoundRobin,
        }
    }
}
impl std::ops::Deref for State {
    type Target = dyn Order;
    fn deref(&self) -> &(dyn Order + 'static) {
        match self {
            Self::InOrder(inner) => inner,
            Self::RoundRobin(inner) => inner,
        }
    }
}
impl std::ops::DerefMut for State {
    fn deref_mut(&mut self) -> &mut (dyn Order + 'static) {
        match self {
            Self::InOrder(inner) => inner,
            Self::RoundRobin(inner) => inner,
        }
    }
}
impl State {
    /// Returns the next element in the ordering
    pub fn next(&mut self, weights: &[Weight]) -> Option<usize> {
        if self.get_weights() != weights {
            self.set_weights(weights);
        }
        let value = self.peek_unchecked();
        self.advance();
        value
    }
    /// Reads what will be returned by call to [`next()`](`Self::next()`)
    pub fn peek(&mut self, weights: &[Weight]) -> Option<usize> {
        if self.get_weights() != weights {
            self.set_weights(weights);
        }
        self.peek_unchecked()
    }
    /// Clears the state, leaving only the [`Type`]
    pub fn clear(&mut self) {
        let ty = Type::from(&*self);
        *self = Self::from(ty);
    }
    /// Sets the order type and clears the state
    pub fn set_type(&mut self, new_ty: Type) {
        if new_ty != Type::from(&*self) {
            *self = Self::from(new_ty);
        }
    }
}
impl PartialEq for State {
    fn eq(&self, other: &State) -> bool {
        Type::from(self) == Type::from(other)
    }
}
impl Eq for State {}
impl std::fmt::Debug for State {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let ty = Type::from(self);
        write!(f, "State::{:?}", ty)
    }
}

/// Supplier of ordering
pub trait Order {
    /// Returns the currently-stored weights array
    fn get_weights(&self) -> &[Weight];
    /// Reads the current value in the ordering
    fn peek_unchecked(&self) -> Option<usize>;
    /// Updates the state for new weights
    fn set_weights(&mut self, new_weights: &[Weight]);
    /// Advances the next element in the ordering
    fn advance(&mut self);
}

/// Tracks weights and items remaining for current index
pub struct InOrderState {
    weights: Vec<Weight>,
    index_remaining: Option<(usize, Weight)>,
}
impl Default for InOrderState {
    fn default() -> Self {
        Self {
            weights: vec![],
            index_remaining: None,
        }
    }
}
impl Order for InOrderState {
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

/// Tracks count remaining for each element
pub struct RoundRobinState {
    weights: Vec<Weight>,
    count: Vec<Weight>,
    index: Option<usize>,
}
impl Default for RoundRobinState {
    fn default() -> Self {
        Self {
            weights: vec![],
            count: vec![],
            index: None,
        }
    }
}
impl Order for RoundRobinState {
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
impl RoundRobinState {
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
    use super::{State, Type, Weight};
    fn assert_peek_next(s: &mut State, weights: &[Weight], expected: Option<usize>) {
        let peeked = s.peek(weights);
        let popped = s.next(weights);
        println!("{:?} = {:?} ??", peeked, expected);
        assert_eq!(peeked, expected);
        assert_eq!(popped, expected);
    }
    fn check_all(ty: Type) {
        check_simple(ty);
        check_blocked(ty);
        check_empty_resizing(ty);
    }
    fn check_simple(ty: Type) {
        let weights = &[1];
        let mut s = State::from(ty);
        for _ in 0..100 {
            assert_peek_next(&mut s, weights, Some(0));
        }
    }
    fn check_blocked(ty: Type) {
        let weights = &[0];
        let mut s = State::from(ty);
        for _ in 0..100 {
            assert_peek_next(&mut s, weights, None);
        }
    }
    fn check_empty_resizing(ty: Type) {
        let weights = &[];
        let mut s = State::from(ty);
        for _ in 0..100 {
            assert_peek_next(&mut s, weights, None);
        }
        //
        let weights = &[1];
        for _ in 0..100 {
            assert_peek_next(&mut s, weights, Some(0));
        }
        //
        let weights = &[0];
        for _ in 0..100 {
            assert_peek_next(&mut s, weights, None);
        }
    }
    // Type::InOrder
    #[test]
    fn in_order_all() {
        let ty = Type::InOrder;
        check_all(ty);
    }
    #[test]
    fn in_order_longer() {
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
    fn in_order_resizing() {
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
    // Type::RoundRobin
    #[test]
    fn round_robin_all() {
        let ty = Type::RoundRobin;
        check_all(ty);
    }
    #[test]
    fn round_robin_longer() {
        let weights = &[1, 2, 2, 3, 0, 5];
        let test_sizes = (0..100).map(|_| weights.len());
        let check_counter = do_run_round_robin(weights, test_sizes);
        assert_eq!(check_counter, 1300); // rigging to ensure test does not get shorter while modifying
    }
    #[test]
    fn round_robin_resizing() {
        let all_weights = &[1, 2, 2, 3, 0, 5];
        let test_sizes = (0..100).map(|i| (i % (all_weights.len() + 1)));
        let check_counter = do_run_round_robin(all_weights, test_sizes);
        assert_eq!(check_counter, 533); // rigging to ensure test does not get shorter while modifying
    }
    #[test]
    fn round_robin_resizing_dynamic() {
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
