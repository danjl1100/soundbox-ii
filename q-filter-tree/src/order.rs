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
//!
//! 3. [`Type::Shuffle`]
//!
//! Shuffles the available nodes, visiting each node proportional to the child's `Weight`.  Weights
//! `[2, 1, 0, 4]` will yield, in some **shuffled** order, two 0's, one 1, no 2's, and four 3's.
//! ```
//! use q_filter_tree::{Tree, error::PopError, OrderType};
//! let mut t: Tree<_, ()> = Tree::default();
//! let root = t.root_id();
//! let mut root_ref = root.try_ref(&mut t).unwrap();
//! //
//! root_ref.set_order(OrderType::Shuffle);
//! //
//! let childA = root_ref.add_child(Some(2));
//! let childB = root_ref.add_child(Some(1));
//! let childC = root_ref.add_child(Some(3));
//! let mut childA_ref = childA.try_ref(&mut t).unwrap();
//! childA_ref.push_item("A");
//! childA_ref.push_item("A");
//! childA_ref.push_item("NEVER");
//! let mut childB_ref = childB.try_ref(&mut t).unwrap();
//! childB_ref.push_item("B");
//! childB_ref.push_item("NEVER");
//! let mut childC_ref = childC.try_ref(&mut t).unwrap();
//! childC_ref.push_item("C");
//! childC_ref.push_item("C");
//! childC_ref.push_item("C");
//! childC_ref.push_item("NEVER");
//! //
//! let mut root_ref = root.try_ref(&mut t).unwrap();
//! let mut popped = vec![];
//! for _ in 0..6 {
//!     popped.push(root_ref.pop_item().unwrap());
//! }
//! // non-deterministic ordering of `popped`, so instead
//! // check some properties of `popped`
//! assert_eq!(popped.iter().filter(|&val| val == &"A").count(), 2);
//! assert_eq!(popped.iter().filter(|&val| val == &"B").count(), 1);
//! assert_eq!(popped.iter().filter(|&val| val == &"C").count(), 3);
//! assert!(popped.iter().filter(|&val| val == &"NEVER").next().is_none());
//! ```

use super::Weight;
use serde::{Deserialize, Serialize};

pub use in_order::State as InOrderState;
mod in_order;

pub use round_robin::State as RoundRobinState;
mod round_robin;

pub use shuffle::State as ShuffleState;
mod shuffle;

/// Method of determining Order
#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Eq, PartialEq, Clone, Copy, Serialize, Deserialize)]
pub enum Type {
    /// Picks [`Weight`] items from one node before moving to the next node
    InOrder,
    /// Picks items from each node in turn, up to maximum of [`Weight`] items per cycle.
    RoundRobin,
    /// Shuffles the order of items given by [`Self::InOrder`] for each cycle.
    Shuffle,
    // TODO
    // /// Randomly selects items based on the relative [`Weight`]s.
    // Random,
}

#[allow(missing_docs)]
#[allow(clippy::large_enum_variant)]
/// State for tracking Ordering progression
pub enum State {
    InOrder(InOrderState),
    RoundRobin(RoundRobinState),
    Shuffle(ShuffleState),
}
impl From<Type> for State {
    fn from(ty: Type) -> Self {
        match ty {
            Type::InOrder => Self::InOrder(InOrderState::default()),
            Type::RoundRobin => Self::RoundRobin(RoundRobinState::default()),
            Type::Shuffle => Self::Shuffle(ShuffleState::default()),
        }
    }
}
impl From<&State> for Type {
    fn from(state: &State) -> Self {
        match state {
            State::InOrder(_) => Self::InOrder,
            State::RoundRobin(_) => Self::RoundRobin,
            State::Shuffle(_) => Self::Shuffle,
        }
    }
}
impl std::ops::Deref for State {
    type Target = dyn Order;
    fn deref(&self) -> &(dyn Order + 'static) {
        match self {
            Self::InOrder(inner) => inner,
            Self::RoundRobin(inner) => inner,
            Self::Shuffle(inner) => inner,
        }
    }
}
impl std::ops::DerefMut for State {
    fn deref_mut(&mut self) -> &mut (dyn Order + 'static) {
        match self {
            Self::InOrder(inner) => inner,
            Self::RoundRobin(inner) => inner,
            Self::Shuffle(inner) => inner,
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

#[cfg(test)]
mod tests {
    use super::{State, Type, Weight};
    pub(super) fn assert_peek_next(s: &mut State, weights: &[Weight], expected: Option<usize>) {
        let peeked = s.peek(weights);
        assert_eq!(s.get_weights(), weights);
        let popped = s.next(weights);
        assert_eq!(s.get_weights(), weights);
        println!("{:?} = {:?} ??", peeked, expected);
        assert_eq!(peeked, expected);
        assert_eq!(popped, expected);
    }
    pub(super) fn check_all(ty: Type) {
        check_type(ty);
        check_simple(ty);
        check_blocked(ty);
        check_empty_resizing(ty);
    }
    fn check_type(ty: Type) {
        let s = State::from(ty);
        let translated_type = Type::from(&s);
        assert_eq!(ty, translated_type);
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
}
