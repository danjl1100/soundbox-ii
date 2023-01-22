// Copyright (C) 2021-2023  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Order of picking nodes from children nodes, given the node [`Weight`]s.
//!
//! # Examples:
//!
//! 1. [`Type::InOrder`]
//!
//! Visits child nodes **in order**.  Weights `[2, 1, 3]` will yield `AABCCC AABCCC ...`
//! ```
//! use std::borrow::Cow;
//! use q_filter_tree::{Tree, OrderType, SequenceAndItem};
//! let mut t: Tree<_, Option<()>> = Tree::default();
//! let root = t.root_id();
//! let mut root_ref = root.try_ref(&mut t);
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
//! let item = |seq, item| SequenceAndItem::new(seq, Cow::Owned(item));
//! //
//! let mut root_ref = root.try_ref(&mut t);
//! assert_eq!(root_ref.pop_item(), Some(item(1, "A1")));
//! assert_eq!(root_ref.pop_item(), Some(item(1, "A2")));
//! assert_eq!(root_ref.pop_item(), Some(item(2, "B1")));
//! assert_eq!(root_ref.pop_item(), Some(item(3, "C1")));
//! assert_eq!(root_ref.pop_item(), Some(item(3, "C2")));
//! assert_eq!(root_ref.pop_item(), Some(item(3, "C3")));
//! assert_eq!(root_ref.pop_item(), None);
//! ```
//!
//! 2. [`Type::RoundRobin`]
//!
//! Cycles through child nodes sequentially, picking one item until reaching each child's `Weight`.  Weights `[2, 1, 3]` will yield `ABCACC ABCACC...`
//! ```
//! use std::borrow::Cow;
//! use q_filter_tree::{Tree, OrderType, SequenceAndItem};
//! let mut t: Tree<_, Option<()>> = Tree::default();
//! let root = t.root_id();
//! let mut root_ref = root.try_ref(&mut t);
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
//! let item = |seq, item| SequenceAndItem::new(seq, Cow::Owned(item));
//! //
//! let mut root_ref = root.try_ref(&mut t);
//! assert_eq!(root_ref.pop_item(), Some(item(1, "A1")));
//! assert_eq!(root_ref.pop_item(), Some(item(2, "B1")));
//! assert_eq!(root_ref.pop_item(), Some(item(3, "C1")));
//! assert_eq!(root_ref.pop_item(), Some(item(1, "A2")));
//! assert_eq!(root_ref.pop_item(), Some(item(3, "C2")));
//! assert_eq!(root_ref.pop_item(), Some(item(3, "C3")));
//! assert_eq!(root_ref.pop_item(), None);
//! ```
//!
//! 3. [`Type::Shuffle`]
//!
//! Shuffles the available nodes, visiting each node proportional to the child's `Weight`.  Weights
//! `[2, 1, 0, 4]` will yield, in some **shuffled** order, two 0's, one 1, no 2's, and four 3's.
//! ```
//! use q_filter_tree::{Tree, OrderType};
//! let mut t: Tree<_, Option<()>> = Tree::default();
//! let root = t.root_id();
//! let mut root_ref = root.try_ref(&mut t);
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
//! let mut root_ref = root.try_ref(&mut t);
//! let mut popped = vec![];
//! for _ in 0..6 {
//!     popped.push(root_ref.pop_item().unwrap().into_item().into_owned());
//! }
//! // non-deterministic ordering of `popped`, so instead
//! // check some properties of `popped`
//! assert_eq!(popped.iter().filter(|&val| val == &"A").count(), 2);
//! assert_eq!(popped.iter().filter(|&val| val == &"B").count(), 1);
//! assert_eq!(popped.iter().filter(|&val| val == &"C").count(), 3);
//! assert!(popped.iter().filter(|&val| val == &"NEVER").next().is_none());
//! ```

#![allow(clippy::module_name_repetitions)]

use crate::weight_vec::{Weight, Weights};
use serde::{Deserialize, Serialize};
use std::ops::Range;

#[cfg(test)]
#[macro_export]
macro_rules! assert_chain {
    (
        // start from Weights (implicit construction)
        let $weights:ident = $weights_expr:expr;
        let $start:ident = $start_expr:expr;
        [
            start => $start_expected:expr;
            $( let $next:ident => $expected:expr; )+
        ]
        $(
            $(weights = $loop_weights:expr;)?
            start = $loop_start:ident;
            [
                start => $loop_start_expected:expr;
                $( let $loop_next:ident => $loop_expected:expr; )+
            ]
        )*
        $(=> $result:expr)?
    ) => {
        {
            let $weights = Weights::from($weights_expr);
            let $start = $start_expr;
            $crate::assert_chain!(@peek $start => $start_expected, start);
            $crate::assert_chain!(@inner $start $weights [ $( let $next => $expected; )+]);

            $(
                $(let weights = Weights::from($loop_weights);)?
                $crate::assert_chain!(@peek $loop_start => $loop_start_expected, start);
                $crate::assert_chain!(@inner $loop_start weights [ $( let $loop_next => $loop_expected; )+ ]);
            )*
            $($result)?
        }
    };
    (@inner $start:ident $weights:ident [ $( let $next:ident => $expected:expr ;)+ ]) => {
        let mut prev = &$start;
        $(
            let $next = prev.next_for(&$weights);
            $crate::assert_chain!(@peek $next => $expected);
            prev = &$next;
        )+
        let _ = prev; // allow(unused) equivalent
    };
    (@peek $current:ident => $expected:expr $(, $label:ident)?) => {
        assert_eq!($current.peek_unchecked(), $expected, stringify!(for $( $label = )? $current));
    };
}

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
#[derive(Default, Debug, Eq, PartialEq, Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(feature = "clap", derive(clap::Subcommand))]
pub enum Type {
    /// Picks [`Weight`] items from one node before moving to the next node
    #[default]
    InOrder,
    /// Picks items from each node in turn, up to maximum of [`Weight`] items per cycle.
    RoundRobin,
    /// Shuffles the order of items given by [`Self::InOrder`] for each cycle.
    Shuffle,
    /// Randomly selects items based on the relative [`Weight`]s.
    Random,
}

#[allow(missing_docs)]
#[allow(clippy::large_enum_variant)]
/// State for tracking Ordering progression
#[derive(Clone)]
pub struct State {
    order: Result<Order, Type>,
}
#[allow(clippy::enum_variant_names)] // TODO: consider renaming `InOrder` to not contain `Order`
#[allow(clippy::large_enum_variant)] // TODO: consider boxing `Shuffle` and `Random`
#[derive(Clone)]
enum Order {
    InOrder(InOrder),
    RoundRobin(RoundRobin),
    Shuffle(Shuffle),
    Random(Random),
}
impl Order {
    fn new(ty: Type, weights: &Weights) -> Self {
        match ty {
            Type::InOrder => Self::InOrder(InOrder::from(weights)),
            Type::RoundRobin => Self::RoundRobin(RoundRobin::from(weights)),
            Type::Shuffle => Self::Shuffle(Shuffle::from(weights)),
            Type::Random => Self::Random(Random::from(weights)),
        }
    }
}
impl From<Type> for State {
    fn from(ty: Type) -> Self {
        Self { order: Err(ty) }
    }
}
impl From<Shuffle> for State {
    fn from(shuffle: Shuffle) -> Self {
        let order = Ok(Order::Shuffle(shuffle));
        Self { order }
    }
}
impl From<&State> for Type {
    fn from(state: &State) -> Self {
        match &state.order {
            Ok(state) => match state {
                Order::InOrder(_) => Self::InOrder,
                Order::RoundRobin(_) => Self::RoundRobin,
                Order::Shuffle(_) => Self::Shuffle,
                Order::Random(_) => Self::Random,
            },
            Err(ty) => *ty,
        }
    }
}
impl std::ops::Deref for Order {
    type Target = dyn OrdererImpl;
    fn deref(&self) -> &(dyn OrdererImpl + 'static) {
        match &self {
            Self::InOrder(inner) => inner,
            Self::RoundRobin(inner) => inner,
            Self::Shuffle(inner) => inner,
            Self::Random(inner) => inner,
        }
    }
}
impl std::ops::DerefMut for Order {
    fn deref_mut(&mut self) -> &mut (dyn OrdererImpl + 'static) {
        match self {
            Self::InOrder(inner) => inner,
            Self::RoundRobin(inner) => inner,
            Self::Shuffle(inner) => inner,
            Self::Random(inner) => inner,
        }
    }
}
impl State {
    /// Returns the next element in the ordering
    pub fn next(&mut self, weights: &Weights) -> Option<usize> {
        let inner = self.inner_mut_advance_or_instantiate(weights);
        inner.peek_unchecked()
    }
    /// Reads what will be returned by call to [`next()`](`Self::next()`)
    pub fn peek(&mut self, weights: &Weights) -> Option<usize> {
        // TODO deleteme, harder to read than triple-nested "IF"s
        // if let Some(valid_peeked) = self.inner().and_then(|inner| {
        //     inner
        //         .peek_unchecked()
        //         .filter(|&index| inner.validate(index, weights))
        // }) {
        //     return Some(valid_peeked);
        // }
        if let Some(inner) = self.inner() {
            if let Some(peeked) = inner.peek_unchecked() {
                if inner.validate(peeked, weights) {
                    return Some(peeked);
                }
            }
        }
        self.next(weights)
    }
    /// Clears the state, leaving only the [`Type`]
    pub fn clear(&mut self) {
        let ty = Type::from(&*self);
        *self = Self::from(ty);
    }
    /// Sets the order type and clears the state
    pub fn set_type(&mut self, new_ty: Type) -> Type {
        let old_ty = Type::from(&*self);
        if new_ty != old_ty {
            *self = Self::from(new_ty);
        }
        old_ty
    }
    fn inner(&self) -> Option<&(dyn OrdererImpl + 'static)> {
        Some(match self.order.as_ref().ok()? {
            Order::InOrder(inner) => inner,
            Order::RoundRobin(inner) => inner,
            Order::Shuffle(inner) => inner,
            Order::Random(inner) => inner,
        })
    }
    fn inner_mut(&mut self) -> Option<&mut (dyn OrdererImpl + 'static)> {
        Some(match self.order.as_mut().ok()? {
            Order::InOrder(inner) => inner,
            Order::RoundRobin(inner) => inner,
            Order::Shuffle(inner) => inner,
            Order::Random(inner) => inner,
        })
    }
    fn inner_mut_advance_or_instantiate(
        &mut self,
        weights: &Weights,
    ) -> &(dyn OrdererImpl + 'static) {
        match &mut self.order {
            Err(ty) => {
                self.order = Ok(Order::new(*ty, weights));
            }
            Ok(order) => order.advance(weights),
        }
        match &self.order {
            Ok(order) => &**order,
            Err(_) => unreachable!("unconditionally set to Ok"),
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
        write!(f, "State::{ty:?}")
    }
}

/// Supplier of ordering
trait OrdererImpl: Orderer {
    /// Reads the current value in the ordering
    fn peek_unchecked(&self) -> Option<usize>;
    /// Validates the specified value for the given weights
    /// Returns `true` if the value is allowed, or `false` if `advance` needs to act
    fn validate(&self, index: usize, weights: &Weights) -> bool;
    /// Advances the next element in the ordering
    fn advance(&mut self, weights: &Weights);
}
/// Externally-facing orderer functions
pub trait Orderer {
    /// Notify that the specified index was removed
    fn notify_removed(&mut self, range: Range<usize>, weights: &Weights);
    /// Notify that the specified weight was changed (or `None`, meaning all indices may have changed)
    fn notify_changed(&mut self, index: Option<usize>, weights: &Weights);
}
impl Orderer for State {
    fn notify_removed(&mut self, range: Range<usize>, weights: &Weights) {
        if let Some(inner) = self.inner_mut() {
            inner.notify_removed(range, weights);
        }
    }

    fn notify_changed(&mut self, index: Option<usize>, weights: &Weights) {
        if let Some(inner) = self.inner_mut() {
            inner.notify_changed(index, weights);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{State, Type, Weight};
    pub(super) use crate::weight_vec::{WeightVec, Weights};

    pub(super) trait CloneToNext {
        fn next_for(&self, weights: &crate::weight_vec::Weights) -> Self;
    }

    pub(super) fn assert_peek_next<T>(
        s: &mut State,
        weight_vec: &WeightVec<T>,
        expected: Option<usize>,
    ) where
        T: std::fmt::Debug,
    {
        let weights = weight_vec.weights();
        let popped = s.next(weights);
        let peeked = s.peek(weights);
        println!("popd {popped:?} = {expected:?} ??");
        println!("peek {peeked:?} = {expected:?} ??");
        assert_eq!(popped, expected);
        assert_eq!(peeked, expected);
    }
    pub(super) fn to_weight_vec(weights: &[Weight]) -> WeightVec<()> {
        weights.iter().copied().map(|w| (w, ())).collect()
    }
    pub(super) fn resize_vec_to_len(
        weight_vec: &mut WeightVec<()>,
        order_state: &mut State,
        target_len: usize,
        all_weights: &[Weight],
    ) {
        let old_len = weight_vec.len();
        let mut weight_vec_ref = weight_vec.ref_mut(order_state);
        weight_vec_ref.truncate(target_len);
        weight_vec_ref.extend(
            all_weights
                .iter()
                .take(target_len)
                .skip(old_len)
                .map(|&w| (w, ())),
        );
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
    pub fn check_truncate(ty: Type) {
        let all_weights = &[1, 1];
        let mut weight_vec = WeightVec::new();
        let mut s = State::from(ty);
        resize_vec_to_len(&mut weight_vec, &mut s, 2, all_weights);
        assert_peek_next(&mut s, &weight_vec, Some(0));
        assert_peek_next(&mut s, &weight_vec, Some(1));
        assert_peek_next(&mut s, &weight_vec, Some(0));
        assert_peek_next(&mut s, &weight_vec, Some(1));
        resize_vec_to_len(&mut weight_vec, &mut s, 1, all_weights);
        for _ in 0..100 {
            assert_peek_next(&mut s, &weight_vec, Some(0));
        }
        resize_vec_to_len(&mut weight_vec, &mut s, 0, all_weights);
        for _ in 0..100 {
            assert_peek_next(&mut s, &weight_vec, None);
        }
    }
}
