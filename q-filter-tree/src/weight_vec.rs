// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Collection types for weighted items
//!
//! See [`OrderVec`] for details of usage.
use serde::{Deserialize, Serialize};
use std::iter::FromIterator;

use crate::{order, OrderType};

/// Numeric type for weighting items in [`WeightVec`], used by to fuel
/// [`OrderType`](`super::OrderType`) algorithms.
pub type Weight = u32;

/// Collection of weights
#[must_use]
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Weights {
    /// Equal (unity) weights of the specified length
    Equal(usize),
    /// Weights
    ///
    /// NOTE: this may contain all unity weights, in the case of roundtrip to/from unity
    Some(Vec<Weight>),
}
impl Default for Weights {
    fn default() -> Self {
        Self::Equal(0)
    }
}
impl Weights {
    /// Returns the length
    #[must_use]
    pub fn len(&self) -> usize {
        match self {
            Self::Equal(len) => *len,
            Self::Some(weights) => weights.len(),
        }
    }
    /// Returns `true` if the weights are empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Equal(len) if *len == 0 => true,
            Self::Some(weights) if weights.is_empty() => true,
            Self::Equal(_) | Self::Some(_) => false,
        }
    }
    const EQUAL_WEIGHT: Weight = 1;
    /// Returns the specified weight element
    #[must_use]
    pub fn get(&self, index: usize) -> Option<Weight> {
        match self {
            Self::Equal(len) => (index < *len).then_some(Self::EQUAL_WEIGHT),
            Self::Some(weights) => weights.get(index).copied(),
        }
    }
    /// Iterates over all weights
    #[must_use]
    pub fn iter(&self) -> Box<dyn Iterator<Item = Weight> + '_> {
        match self {
            Self::Equal(len) => Box::new(std::iter::repeat(Self::EQUAL_WEIGHT).take(*len)),
            Self::Some(weights) => Box::new(weights.iter().copied()),
        }
    }
    /// Returns a mutable reference to the specified weight
    ///
    /// NOTE: This transforms in-place on the first modification
    fn get_mut(&mut self, index: usize) -> Option<&mut Weight> {
        if index < self.len() {
            match self {
                Self::Equal(len) => {
                    *self = Self::Some(vec![Self::EQUAL_WEIGHT; *len]);
                    match self {
                        Self::Some(weights) => weights.get_mut(index),
                        Self::Equal(_) => unreachable!(),
                    }
                }
                Self::Some(weights) => weights.get_mut(index),
            }
        } else {
            None
        }
    }
    /// Appends the specified weight
    ///
    /// NOTE: This transitions in-place on the first non-unity value added (`value != 1`)
    pub fn push(&mut self, value: Weight) {
        match self {
            Self::Equal(len) if value == Self::EQUAL_WEIGHT => {
                *len += 1;
            }
            Self::Equal(len) => {
                let mut weights = vec![Self::EQUAL_WEIGHT; *len + 1];
                weights[*len] = value;
                *self = Self::Some(weights);
            }
            Self::Some(weights) => weights.push(value),
        }
    }
    /// Attempts to remove the last weight element
    pub fn pop(&mut self) -> Option<Weight> {
        match self {
            Self::Equal(0) => None,
            Self::Equal(len) => {
                *len -= 1;
                Some(Self::EQUAL_WEIGHT)
            }
            Self::Some(weights) => weights.pop(),
        }
    }
    /// Attempts to remove the weight element at the specified index
    ///
    /// # Panics
    /// Panics if the specified `index` is out of bounds
    #[allow(clippy::panic)] // allowed in Vec::remove calls, so allow on extension enum
    pub fn remove(&mut self, index: usize) -> Weight {
        match self {
            Self::Equal(len) if index < *len => {
                *len -= 1;
                Self::EQUAL_WEIGHT
            }
            Self::Equal(len) => panic!("requested index {index} is out of bounds of length {len}"),
            Self::Some(weights) => weights.remove(index),
        }
    }
    /// Truncates the weights to the specified maximum length
    pub fn truncate(&mut self, new_len: usize) {
        match self {
            Self::Equal(len) => {
                *len = new_len.min(*len);
            }
            Self::Some(weights) => weights.truncate(new_len),
        }
    }
    /// Checks the weights, and simplifies to `Equal` if they are all unity
    pub fn try_simplify(&mut self) {
        match self {
            Self::Some(weights) if weights.iter().all(|w| *w == Self::EQUAL_WEIGHT) => {
                *self = Self::Equal(weights.len());
            }
            Self::Equal(_) | Self::Some(_) => {}
        }
    }
}
impl<'a> IntoIterator for &'a Weights {
    type Item = Weight;
    type IntoIter = Box<dyn Iterator<Item = Weight> + 'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
impl From<Vec<Weight>> for Weights {
    fn from(weights: Vec<Weight>) -> Self {
        let mut weights = Self::Some(weights);
        weights.try_simplify();
        weights
    }
}

/// Collection of weighted items
#[must_use]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WeightVec<T>(Weights, Vec<T>);
impl<T> WeightVec<T> {
    /// Creates a new empty collection
    pub fn new() -> Self {
        Self(Weights::Equal(0), vec![])
    }
    /// Returns the length
    #[must_use]
    pub fn len(&self) -> usize {
        let len = self.0.len();
        debug_assert_eq!(len, self.1.len());
        len
    }
    /// Returns `true` if the collection is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        let is_empty = self.0.is_empty();
        debug_assert_eq!(is_empty, self.1.is_empty());
        is_empty
    }
    /// Returns the specified weight and element
    #[must_use]
    pub fn get(&self, index: usize) -> Option<(Weight, &T)> {
        match (self.0.get(index), self.1.get(index)) {
            (Some(weight), Some(node)) => Some((weight, node)),
            _ => None,
        }
    }
    /// Returns all weights
    pub fn weights(&self) -> &Weights {
        &self.0
    }
    /// Returns all elements
    #[must_use]
    pub fn elems(&self) -> &[T] {
        &self.1
    }
    /// Returns the specified mutable element
    #[must_use]
    pub fn get_elem_mut(&mut self, index: usize) -> Option<&mut T> {
        self.1.get_mut(index)
    }
    /// Iterates the weights and items
    pub fn iter(&self) -> impl Iterator<Item = (Weight, &T)> {
        self.weights().iter().zip(self.elems().iter())
    }
    // TODO remove if not needed
    // pub fn iter_mut_elems_straight(&mut self) -> impl Iterator<Item = &mut T> {
    //     self.1.iter_mut()
    // }
    pub(super) fn ref_mut<'order>(
        &mut self,
        order: &'order mut order::State,
    ) -> RefMut<'_, 'order, T> {
        self.ref_mut_optional(Some(order))
    }
    fn ref_mut_optional<'order>(
        &mut self,
        order: Option<&'order mut order::State>,
    ) -> RefMut<'_, 'order, T> {
        RefMut {
            weights: &mut self.0,
            elems: &mut self.1,
            order,
        }
    }
    /// Destructures into the weights and items
    pub fn into_parts(self) -> (Weights, Vec<T>) {
        (self.0, self.1)
    }
    /// Returns the next index for the specified order state
    pub fn next_index_with_order(&self, order: &mut order::State) -> Option<usize> {
        order.next(&self.0)
    }
    /// Returns the next item for the specified order state
    ///
    /// # Errors
    /// Returns an error if the specified order state is outside the bounds of the vector
    fn next_with_order(&self, order: &mut order::State) -> Result<Option<&T>, (usize, usize)> {
        if let Some(index) = self.next_index_with_order(order) {
            if let Some(item) = self.1.get(index) {
                Ok(Some(item))
            } else {
                Err((index, self.len()))
            }
        } else {
            Ok(None)
        }
    }
    /// Returns the next item (mut reference) for the specified order state
    ///
    /// # Errors
    /// Returns an error if the specified order state is outside the bounds of the vector
    fn next_with_order_mut(
        &mut self,
        order: &mut order::State,
    ) -> Result<Option<&mut T>, (usize, usize)> {
        if let Some(index) = self.next_index_with_order(order) {
            let self_len = self.len();
            if let Some(item) = self.1.get_mut(index) {
                Ok(Some(item))
            } else {
                Err((index, self_len))
            }
        } else {
            Ok(None)
        }
    }
}
impl<T> Default for WeightVec<T> {
    fn default() -> Self {
        Self::new()
    }
}
impl<T> FromIterator<(Weight, T)> for WeightVec<T> {
    fn from_iter<U: IntoIterator<Item = (Weight, T)>>(iter: U) -> Self {
        let mut weight_vec = WeightVec::new();
        {
            let mut ref_mut = weight_vec.ref_mut_optional(None);
            for item in iter {
                ref_mut.push(item);
            }
        }
        weight_vec
    }
}

/// Mutable handle to a [`WeightVec`], optionally updating an order state on each action
pub struct RefMut<'vec, 'order, T> {
    weights: &'vec mut Weights,
    elems: &'vec mut Vec<T>,
    order: Option<&'order mut order::State>,
}
impl<'vec, 'order, T> RefMut<'vec, 'order, T> {
    /// Sets the weight of the specified index
    ///
    /// # Errors
    /// Returns an error if the index if out of bounds
    pub fn set_weight(&mut self, index: usize, new_weight: Weight) -> Result<(), usize> {
        if let Some(weight) = self.weights.get_mut(index) {
            // changed
            *weight = new_weight;
            if let Some(order) = &mut self.order {
                order.notify_changed(Some(index), self.weights);
            }
            Ok(())
        } else {
            Err(index)
        }
    }
    /// Returns a mutable reference to the specified element
    pub fn get_elem_mut(&mut self, index: usize) -> Option<&mut T> {
        // no update needed - weight not changed
        self.elems.get_mut(index)
    }
    /// Appends an element
    pub fn push(&mut self, (weight, item): (Weight, T)) {
        // no updated needed - not removed, nor changed
        self.weights.push(weight);
        self.elems.push(item);
    }
    /// Removes the last element
    ///
    /// # Panics
    /// Panics if the internal state is inconsistent (library implementation logic error)
    pub fn pop(&mut self) -> Option<(Weight, T)> {
        assert_eq!(
            self.weights.len(),
            self.elems.len(),
            "weights and items lists length equal before pop"
        );
        match (self.weights.pop(), self.elems.pop()) {
            (Some(weight), Some(elem)) => Some((weight, elem)),
            (None, None) => None,
            _ => unreachable!("equal length weights/elems Vecs pop equivalently"),
        }
    }
    /// Removes the specified element
    ///
    /// # Errors
    /// Returns an error if the specified index is out of bounds
    ///
    /// # Panics
    /// Panics if the internal state is inconsistent (library implementation logic error)
    pub fn remove(&mut self, index: usize) -> Result<(Weight, T), usize> {
        assert_eq!(
            self.weights.len(),
            self.elems.len(),
            "weights and items lists length equal before removal"
        );
        if index < self.weights.len() {
            let removed = (self.weights.remove(index), self.elems.remove(index));
            if let Some(order) = &mut self.order {
                #[allow(clippy::range_plus_one)]
                let singleton_range = index..(index + 1);
                order.notify_removed(singleton_range, self.weights);
            }
            Ok(removed)
        } else {
            Err(index)
        }
    }
    /// Shortens the collection to the specified length
    ///
    /// # Panics
    /// Panics if the internal state is inconsistent (library implementation logic error)
    pub fn truncate(&mut self, length: usize) {
        assert_eq!(
            self.weights.len(),
            self.elems.len(),
            "weights and items lists length equal before truncation"
        );
        let orig_len = self.weights.len();
        if length < orig_len {
            self.weights.truncate(length);
            self.elems.truncate(length);
            if let Some(order) = &mut self.order {
                order.notify_removed(length..orig_len, self.weights);
            }
        }
    }
    /// Sets the weights to the specified values.
    ///
    /// *Overwrite* triggers a reset of the order state (if any).
    ///
    /// # Errors
    /// Returns and error if the length of supplied weights do not match the existing length
    ///
    /// # Panics
    /// Panics if the internal state is inconsistent (library implementation logic error)
    pub fn overwrite_weights(&mut self, weights: Weights) -> Result<(), (Weights, usize)> {
        let orig_len = self.weights.len();
        if weights.len() == orig_len {
            *self.weights = weights;
            if let Some(order) = &mut self.order {
                order.notify_changed(None, self.weights);
            }
            assert_eq!(
                self.weights.len(),
                self.elems.len(),
                "weights and items lists length equal after overwrite_child_weights"
            );
            Ok(())
        } else {
            Err((weights, orig_len))
        }
    }
    /// Creates a mutable handle to the element at the specified index
    ///
    /// # Errors
    /// Returns an error if the specified index is out of bounds
    ///
    /// # Panics
    /// Panics if the internal state is inconsistent (library implementation logic error)
    pub fn into_elem_ref(
        self,
        index: usize,
    ) -> Result<(RefMutWeight<'vec, 'order>, &'vec mut T), usize> {
        assert_eq!(
            self.weights.len(),
            self.elems.len(),
            "weights and items lists length prior to into_elem_ref"
        );
        let Self {
            weights,
            elems,
            order,
        } = self;
        elems
            .get_mut(index)
            .map(move |elem| {
                (
                    RefMutWeight {
                        weights,
                        order,
                        index,
                    },
                    elem,
                )
            })
            .ok_or(index)
    }
}

/// Mutable handle to a specific element
pub type RefMutElem<'vec, 'order, T> = (RefMutWeight<'vec, 'order>, &'vec mut T);
/// Mutable handle to a weight
pub struct RefMutWeight<'vec, 'order> {
    weights: &'vec mut Weights,
    order: Option<&'order mut order::State>,
    index: usize,
}
impl<'vec, 'order> RefMutWeight<'vec, 'order> {
    /// Sets the weight to the specified value
    pub fn set_weight(&mut self, weight: Weight) {
        let weight_mut = self
            .weights
            .get_mut(self.index)
            .expect("valid index in created WeightVecMutElem");
        *weight_mut = weight;
        if let Some(order) = &mut self.order {
            order.notify_changed(Some(self.index), self.weights);
        }
    }
    /// Returns the current weight value
    #[must_use]
    pub fn get_weight(&self) -> Weight {
        self.weights
            .get(self.index)
            .expect("valid index in created WeightVecMutElem")
    }
}

/// Collection of weighted items, with an order state for organized iteration
///
/// ```
/// use q_filter_tree::{OrderType, weight_vec::OrderVec};
/// let mut v = OrderVec::new(OrderType::InOrder);
/// let mut v_ref = v.ref_mut();
/// v_ref.push((2, "first"));
/// v_ref.push((3, "second"));
/// v_ref.push((1, "last"));
/// assert_eq!(v.next_item(), Some(&"first"));
/// assert_eq!(v.next_item(), Some(&"first"));
/// assert_eq!(v.next_item(), Some(&"second"));
/// assert_eq!(v.next_item(), Some(&"second"));
/// assert_eq!(v.next_item(), Some(&"second"));
/// assert_eq!(v.next_item(), Some(&"last"));
/// //
/// v.set_order(OrderType::RoundRobin);
/// assert_eq!(v.next_item(), Some(&"first"));
/// assert_eq!(v.next_item(), Some(&"second"));
/// assert_eq!(v.next_item(), Some(&"last"));
/// assert_eq!(v.next_item(), Some(&"first"));
/// assert_eq!(v.next_item(), Some(&"second"));
/// assert_eq!(v.next_item(), Some(&"second"));
/// ///
/// v.ref_mut().truncate(1);
/// for _ in 0..100 {
///     assert_eq!(v.next_item(), Some(&"first"));
/// }
/// ```
#[must_use]
#[derive(Clone)]
pub struct OrderVec<T> {
    order: order::State,
    vec: WeightVec<T>,
}
impl<T> OrderVec<T> {
    /// Creates a new `OrderVec` with the specified order type
    pub fn new(ty: order::Type) -> Self {
        let order = order::State::from(ty);
        let vec = WeightVec::new();
        Self { order, vec }
    }
    /// Returns the type of the order state
    #[must_use]
    pub fn get_order_type(&self) -> order::Type {
        order::Type::from(&self.order)
    }
    /// Returns a mutable handle to the collection
    pub fn ref_mut(&mut self) -> RefMut<'_, '_, T> {
        self.vec.ref_mut(&mut self.order)
    }
    /// Returns a mutable reference to the specified element
    pub fn get_elem_mut(&mut self, index: usize) -> Option<&mut T> {
        self.vec.get_elem_mut(index)
    }
    // TODO remove if not needed
    // pub fn iter_mut_elems_straight(&mut self) -> impl Iterator<Item = &mut T> {
    //     self.vec.iter_mut_elems_straight()
    // }
    /// Sets the [`OrderType`](`crate::order::Type`)
    pub fn set_order(&mut self, ty: order::Type) {
        self.order.set_type(ty);
    }
    /// Destructures into the weight and item components
    pub fn into_parts(self) -> (order::Type, (Weights, Vec<T>)) {
        ((&self.order).into(), self.vec.into_parts())
    }
    /// Returns the next element in the order
    pub fn next_item(&mut self) -> Option<&T> {
        self.vec
            .next_with_order(&mut self.order)
            .expect("order-provided index out of bounds")
    }
    /// Returns a mutable reference to the next element in the order
    pub fn next_item_mut(&mut self) -> Option<&mut T> {
        self.vec
            .next_with_order_mut(&mut self.order)
            .expect("order-provided index out of bounds")
    }
    pub(crate) fn next_index(&mut self) -> Option<usize> {
        self.vec.next_index_with_order(&mut self.order)
    }
}
// NOTE: impl only Deref, provide custom methods for `mut` methods, to wrap in order::State
impl<T> std::ops::Deref for OrderVec<T> {
    type Target = WeightVec<T>;

    fn deref(&self) -> &Self::Target {
        &self.vec
    }
}
impl<T> std::cmp::PartialEq for OrderVec<T>
where
    T: std::cmp::PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.get_order_type() == other.get_order_type() && self.vec == other.vec
    }
}
impl<T> std::cmp::Eq for OrderVec<T> where T: std::cmp::Eq {}
impl<T, I> From<(OrderType, I)> for OrderVec<T>
where
    I: IntoIterator<Item = (Weight, T)>,
{
    fn from((ty, iter): (OrderType, I)) -> Self {
        let order = ty.into();
        let vec = WeightVec::from_iter(iter);
        Self { order, vec }
    }
}
impl<'a, 'b, T> Extend<(Weight, T)> for RefMut<'a, 'b, T> {
    fn extend<I: IntoIterator<Item = (Weight, T)>>(&mut self, iter: I) {
        for elem in iter {
            self.push(elem);
        }
    }
}
impl<T> Extend<(Weight, T)> for OrderVec<T> {
    fn extend<I: IntoIterator<Item = (Weight, T)>>(&mut self, iter: I) {
        self.ref_mut().extend(iter);
    }
}
