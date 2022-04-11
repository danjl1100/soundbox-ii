// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
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
//! root_ref.set_order_type(OrderType::InOrder);
//! //
//! let mut root_ref = root_ref.child_nodes().unwrap();
//! let childA = root_ref.add_child(2);
//! let childB = root_ref.add_child(1);
//! let childC = root_ref.add_child(3);
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
//! assert_eq!(root_ref.pop_item_queued(), Some("A1"));
//! assert_eq!(root_ref.pop_item_queued(), Some("A2"));
//! assert_eq!(root_ref.pop_item_queued(), Some("B1"));
//! assert_eq!(root_ref.pop_item_queued(), Some("C1"));
//! assert_eq!(root_ref.pop_item_queued(), Some("C2"));
//! assert_eq!(root_ref.pop_item_queued(), Some("C3"));
//! assert_eq!(root_ref.pop_item_queued(), None);
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
//! root_ref.set_order_type(OrderType::RoundRobin);
//! //
//! let mut root_ref = root_ref.child_nodes().unwrap();
//! let childA = root_ref.add_child(2);
//! let childB = root_ref.add_child(1);
//! let childC = root_ref.add_child(3);
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
//! assert_eq!(root_ref.pop_item_queued(), Some("A1"));
//! assert_eq!(root_ref.pop_item_queued(), Some("B1"));
//! assert_eq!(root_ref.pop_item_queued(), Some("C1"));
//! assert_eq!(root_ref.pop_item_queued(), Some("A2"));
//! assert_eq!(root_ref.pop_item_queued(), Some("C2"));
//! assert_eq!(root_ref.pop_item_queued(), Some("C3"));
//! assert_eq!(root_ref.pop_item_queued(), None);
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
//! root_ref.set_order_type(OrderType::Shuffle);
//! //
//! let mut root_ref = root_ref.child_nodes().unwrap();
//! let childA = root_ref.add_child(2);
//! let childB = root_ref.add_child(1);
//! let childC = root_ref.add_child(3);
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
//!     popped.push(root_ref.pop_item_queued().unwrap());
//! }
//! // non-deterministic ordering of `popped`, so instead
//! // check some properties of `popped`
//! assert_eq!(popped.iter().filter(|&val| val == &"A").count(), 2);
//! assert_eq!(popped.iter().filter(|&val| val == &"B").count(), 1);
//! assert_eq!(popped.iter().filter(|&val| val == &"C").count(), 3);
//! assert!(popped.iter().filter(|&val| val == &"NEVER").next().is_none());
//! ```

#![allow(clippy::module_name_repetitions)]

use super::Weight;
use serde::{Deserialize, Serialize};

pub use in_order::InOrder;
mod in_order;

pub use round_robin::RoundRobin;
mod round_robin;

pub use shuffle::Shuffle;
mod shuffle;

pub use random::Random;
mod random;

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
    /// Randomly selects items based on the relative [`Weight`]s.
    Random,
}
impl Default for Type {
    fn default() -> Self {
        Self::InOrder
    }
}

#[allow(missing_docs)]
#[allow(clippy::large_enum_variant)]
/// State for tracking Ordering progression
#[derive(Clone)]
pub struct State {
    order: Order,
}
#[allow(clippy::enum_variant_names)] // TODO: consider renaming `InOrder` to not contain `Order`
#[allow(clippy::large_enum_variant)] // TODO: consider boxing `Shuffle`
#[derive(Clone)]
enum Order {
    InOrder(InOrder),
    RoundRobin(RoundRobin),
    Shuffle(Shuffle),
    Random(Random),
}
impl From<Type> for State {
    fn from(ty: Type) -> Self {
        let order = match ty {
            Type::InOrder => Order::InOrder(InOrder::default()),
            Type::RoundRobin => Order::RoundRobin(RoundRobin::default()),
            Type::Shuffle => Order::Shuffle(Shuffle::default()),
            Type::Random => Order::Random(Random::default()),
        };
        Self { order }
    }
}
impl From<Shuffle> for State {
    fn from(shuffle: Shuffle) -> Self {
        let order = Order::Shuffle(shuffle);
        Self { order }
    }
}
impl From<&State> for Type {
    fn from(state: &State) -> Self {
        match state.order {
            Order::InOrder(_) => Self::InOrder,
            Order::RoundRobin(_) => Self::RoundRobin,
            Order::Shuffle(_) => Self::Shuffle,
            Order::Random(_) => Self::Random,
        }
    }
}
impl std::ops::Deref for State {
    type Target = dyn Orderer;
    fn deref(&self) -> &(dyn Orderer + 'static) {
        match &self.order {
            Order::InOrder(inner) => inner,
            Order::RoundRobin(inner) => inner,
            Order::Shuffle(inner) => inner,
            Order::Random(inner) => inner,
        }
    }
}
impl std::ops::DerefMut for State {
    fn deref_mut(&mut self) -> &mut (dyn Orderer + 'static) {
        match &mut self.order {
            Order::InOrder(inner) => inner,
            Order::RoundRobin(inner) => inner,
            Order::Shuffle(inner) => inner,
            Order::Random(inner) => inner,
        }
    }
}
impl State {
    /// Returns the next element in the ordering
    pub fn next(&mut self, weights: &[Weight]) -> Option<usize> {
        let prev_value = self.peek(weights);
        self.advance(weights);
        prev_value
    }
    /// Reads what will be returned by call to [`next()`](`Self::next()`)
    pub fn peek(&mut self, weights: &[Weight]) -> Option<usize> {
        let valid_range = 0..weights.len();
        match self.peek_unchecked() {
            Some(index) if valid_range.contains(&index) => Some(index),
            Some(_) | None => {
                self.advance(weights);
                self.peek_unchecked()
            }
        }
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
pub trait Orderer {
    /// Reads the current value in the ordering
    fn peek_unchecked(&self) -> Option<usize>;
    /// Advances the next element in the ordering
    fn advance(&mut self, weights: &[Weight]);
    /// Notify that the specified index was removed
    fn notify_removed(&mut self, index: usize, weights: &[Weight]);
    /// Notify that the specified weight was changed (or `None`, meaning all indices may have changed)
    fn notify_changed(&mut self, index: Option<usize>, weights: &[Weight]);
}

#[cfg(test)]
mod tests {
    use super::{State, Type, Weight};
    pub(super) use crate::weight_vec::WeightVec;
    pub(super) fn assert_peek_next<T>(
        s: &mut State,
        weight_vec: &WeightVec<T>,
        expected: Option<usize>,
    ) where
        T: std::fmt::Debug,
    {
        let weights = weight_vec.weights();
        let peeked = s.peek(weights);
        // TODO is a check like this needed??? assert_eq!(s.get_weights(), weights);
        let popped = s.next(weights);
        // TODO is a check like this needed??? assert_eq!(s.get_weights(), weights);
        println!("{:?} = {:?} ??", peeked, expected);
        assert_eq!(peeked, expected);
        assert_eq!(popped, expected);
    }
    pub(super) fn to_weight_vec(weights: &[Weight]) -> WeightVec<()> {
        weights.iter().copied().map(|w| (w, ())).collect()
    }
    pub(super) fn resize_vec_to_len(
        weight_vec: &mut WeightVec<()>,
        order_state: &mut State,
        target_len: usize,
        all_weights: &[super::Weight],
    ) {
        while weight_vec.len() > target_len {
            weight_vec.ref_mut(order_state).pop();
        }
        while weight_vec.len() < target_len {
            let weight = all_weights[weight_vec.len()];
            weight_vec.ref_mut(order_state).push((weight, ()));
        }
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
        let weight_vec = to_weight_vec(&[1]);
        let mut s = State::from(ty);
        for _ in 0..100 {
            assert_peek_next(&mut s, &weight_vec, Some(0));
        }
    }
    fn check_blocked(ty: Type) {
        let weight_vec = to_weight_vec(&[0]);
        let mut s = State::from(ty);
        for _ in 0..100 {
            assert_peek_next(&mut s, &weight_vec, None);
        }
    }
    fn check_empty_resizing(ty: Type) {
        let mut weights = to_weight_vec(&[]);
        let mut s = State::from(ty);
        for _ in 0..100 {
            assert_peek_next(&mut s, &weights, None);
        }
        // [1]
        weights.ref_mut(&mut s).push((1, ()));
        for _ in 0..100 {
            assert_peek_next(&mut s, &weights, Some(0));
        }
        // [0]
        weights
            .ref_mut(&mut s)
            .set_weight(0, 0)
            .expect("index in bounds");
        for _ in 0..100 {
            assert_peek_next(&mut s, &weights, None);
        }
    }
}
