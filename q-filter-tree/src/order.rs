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
//! //
//! t.set_order(&root, OrderType::InOrder);
//! //
//! let childA = t.add_child(&root, Some(2)).unwrap();
//! t.push_item(&childA, "A1").unwrap();
//! t.push_item(&childA, "A2").unwrap();
//! let childB = t.add_child(&root, Some(1)).unwrap();
//! t.push_item(&childB, "B1").unwrap();
//! let childC = t.add_child(&root, Some(3)).unwrap();
//! t.push_item(&childC, "C1").unwrap();
//! t.push_item(&childC, "C2").unwrap();
//! t.push_item(&childC, "C3").unwrap();
//! //
//! assert_eq!(t.pop_item_from(&root).unwrap(), Ok("A1"));
//! assert_eq!(t.pop_item_from(&root).unwrap(), Ok("A2"));
//! assert_eq!(t.pop_item_from(&root).unwrap(), Ok("B1"));
//! assert_eq!(t.pop_item_from(&root).unwrap(), Ok("C1"));
//! assert_eq!(t.pop_item_from(&root).unwrap(), Ok("C2"));
//! assert_eq!(t.pop_item_from(&root).unwrap(), Ok("C3"));
//! assert_eq!(t.pop_item_from(&root).unwrap(), Err(PopError::Empty(root.into())));
//! ```
//!
//! 2. [`Type::RoundRobin`]
//!
//! Cycles through child nodes sequentially, picking one item until reaching each child's `Weight`.  Weights `[2, 1, 3]` will yield `ABCACC ABCACC...`
//! ```
//! use q_filter_tree::{Tree, error::PopError, OrderType};
//! let mut t: Tree<_, ()> = Tree::default();
//! let root = t.root_id();
//! //
//! t.set_order(&root, OrderType::RoundRobin);
//! //
//! let childA = t.add_child(&root, Some(2)).unwrap();
//! t.push_item(&childA, "A1").unwrap();
//! t.push_item(&childA, "A2").unwrap();
//! let childB = t.add_child(&root, Some(1)).unwrap();
//! t.push_item(&childB, "B1").unwrap();
//! let childC = t.add_child(&root, Some(3)).unwrap();
//! t.push_item(&childC, "C1").unwrap();
//! t.push_item(&childC, "C2").unwrap();
//! t.push_item(&childC, "C3").unwrap();
//! //
//! assert_eq!(t.pop_item_from(&root).unwrap(), Ok("A1"));
//! assert_eq!(t.pop_item_from(&root).unwrap(), Ok("B1"));
//! assert_eq!(t.pop_item_from(&root).unwrap(), Ok("C1"));
//! assert_eq!(t.pop_item_from(&root).unwrap(), Ok("A2"));
//! assert_eq!(t.pop_item_from(&root).unwrap(), Ok("C2"));
//! assert_eq!(t.pop_item_from(&root).unwrap(), Ok("C3"));
//! assert_eq!(t.pop_item_from(&root).unwrap(), Err(PopError::Empty(root.into())));
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
            self.advance(Some(weights));
        }
        let value = self.peek_unchecked();
        self.advance(None);
        value
    }
    /// Reads what will be returned by call to [`next()`](`Self::next()`)
    pub fn peek(&mut self, weights: &[Weight]) -> Option<usize> {
        if self.get_weights() != weights {
            self.advance(Some(weights));
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
    /// Advances the next element in the ordering
    fn advance(&mut self, resize_weights: Option<&[Weight]>);
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
    fn advance(&mut self, resize: Option<&[Weight]>) {
        if let Some(new_weights) = resize {
            self.weights = new_weights.to_vec();
            self.index_remaining = None;
        }
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
    count_remaining: Vec<Weight>,
    index: Option<usize>,
    peek_value: Option<usize>, //TODO remove redundancy, code smell
}
impl Default for RoundRobinState {
    fn default() -> Self {
        Self {
            weights: vec![],
            count_remaining: vec![],
            index: None,
            peek_value: None,
        }
    }
}
impl Order for RoundRobinState {
    fn get_weights(&self) -> &[Weight] {
        &self.weights
    }
    fn advance(&mut self, resize: Option<&[Weight]>) {
        if let Some(new_weights) = resize {
            self.weights = new_weights.to_vec();
        }
        self.peek_value = if self.weights.is_empty() || self.weights.iter().all(|x| *x == 0) {
            None
        } else {
            let weights_len = self.weights.len();
            let mut mark_no_progress_since = None;
            loop {
                // fill count_remaining
                if self.count_remaining.is_empty() {
                    self.count_remaining = self.weights.clone();
                }
                // increment
                let index = match self.index {
                    Some(prev_index) if prev_index + 1 < weights_len => prev_index + 1,
                    Some(_) | None if 0 < weights_len => 0,
                    _ => {
                        // no valid index
                        break None;
                    }
                };
                self.index.replace(index);
                // catch full-loop-no-progress
                match mark_no_progress_since {
                    Some(i) if i == index => {
                        mark_no_progress_since = None;
                        // reset
                        self.index = None;
                        self.count_remaining.clear();
                        continue;
                    }
                    _ => {}
                }
                // check count-remaining
                match self.count_remaining.get_mut(index) {
                    Some(0) => {
                        // record "no progress" marker
                        if mark_no_progress_since.is_none() {
                            mark_no_progress_since.replace(index);
                        }
                        continue;
                    }
                    Some(count) => {
                        // found! decrement
                        *count -= 1;
                        break Some(index);
                    }
                    None => unreachable!("length mismatch: self.count_remaining to self.weights"),
                }
            }
        }
    }
    fn peek_unchecked(&self) -> Option<usize> {
        // impl not clear from function above
        self.peek_value
    }
}

#[cfg(test)]
mod tests {
    use super::{State, Type};
    fn check_simple(ty: Type) {
        let weights = &[1];
        let mut s = State::from(ty);
        for _ in 0..100 {
            assert_eq!(s.peek(weights), Some(0));
            assert_eq!(s.next(weights), Some(0));
        }
    }
    fn check_blocked(ty: Type) {
        let weights = &[0];
        let mut s = State::from(ty);
        for _ in 0..100 {
            assert_eq!(s.peek(weights), None);
            assert_eq!(s.next(weights), None);
        }
    }
    // Type::InOrder
    #[test]
    fn in_order_simple_and_blocked() {
        let ty = Type::InOrder;
        check_simple(ty);
        check_blocked(ty);
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
                    assert_eq!(s.peek(weights), Some(index));
                    assert_eq!(s.next(weights), Some(index));
                    //
                    // let value = s.next(weights);
                    // let expected = Some(index);
                    // assert_eq!(value, expected);
                    // println!("{:?} = {:?} ??", value, expected);
                }
            }
        }
    }
    // Type::RoundRobin
    #[test]
    fn round_robin_simple_and_blocked() {
        let ty = Type::RoundRobin;
        //
        check_simple(ty);
        check_blocked(ty);
    }
    #[test]
    fn round_robin_longer() {
        let ty = Type::RoundRobin;
        //
        let weights = &[1, 2, 2, 3, 0, 5];
        let mut s = State::from(ty);
        for _ in 0..100 {
            let mut remaining = weights.to_vec();
            loop {
                let mut popped = false;
                for (index, remaining) in remaining.iter_mut().enumerate() {
                    if *remaining > 0 {
                        popped = true;
                        *remaining -= 1;
                        //
                        assert_eq!(s.peek(weights), Some(index));
                        assert_eq!(s.next(weights), Some(index));
                        //
                        // let value = s.next(weights);
                        // let expected = Some(index);
                        // assert_eq!(value, expected);
                        // println!("{:?} = {:?} ??", value, expected);
                    }
                }
                if !popped {
                    break;
                }
            }
        }
    }
}
