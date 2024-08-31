// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use super::RandResult;
use crate::{ChildVec, Weights};
use arbitrary::Unstructured;

pub(super) trait OrderSource<R: rand::Rng + ?Sized> {
    /// Returns the next index in the order, within the range `0..=max_index`
    fn next(&mut self, rng: &mut R, weights: Weights<'_>) -> RandResult<usize>;
    /// Returns the next index in the order to index the specified target slice
    /// or `None` if the specified `target` is empty.
    fn next_in_equal<T>(&mut self, rng: &mut R, target: &[T]) -> Option<RandResult<usize>> {
        let weights = Weights::new_equal(target.len())?;
        let next = self.next(rng, weights);
        Some(next)
    }
    /// Returns the next index in the order to index the specified target [`ChildVec`],
    /// or `None` if the specified `target` is empty.
    fn next_in<T>(&mut self, rng: &mut R, target: &ChildVec<T>) -> Option<RandResult<usize>> {
        let weights = target.weights()?;
        let next = self.next(rng, weights);
        Some(next)
    }
}

/// Ordering scheme for child nodes of a joint, or child items of a bucket
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum OrderType {
    /// Selects each child in turn, repeating each according to the weights
    #[default]
    InOrder,
    /// Selects a random (weighted) child
    Random,
    /// Selects from a randomized order of the children
    /// NOTE: For N total child-weight choices, the result is the shuffled version of
    /// [`InOrder`](`Self::InOrder`)
    Shuffle,
}
impl std::fmt::Display for OrderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            OrderType::InOrder => "in order",
            OrderType::Random => "random",
            OrderType::Shuffle => "shuffle",
        };
        write!(f, "{name}")
    }
}

#[derive(Clone, Debug)]
#[allow(clippy::enum_variant_names)]
pub(super) enum Order {
    InOrder(InOrder),
    Random(Random),
    Shuffle(Shuffle),
}
impl Default for Order {
    fn default() -> Self {
        Self::new(OrderType::default())
    }
}
impl Order {
    pub(super) fn new(ty: OrderType) -> Self {
        match ty {
            OrderType::InOrder => Self::InOrder(InOrder::default()),
            OrderType::Random => Self::Random(Random::default()),
            OrderType::Shuffle => Self::Shuffle(Shuffle::default()),
        }
    }
    pub(super) fn get_ty(&self) -> OrderType {
        match self {
            Order::InOrder(_) => OrderType::InOrder,
            Order::Random(_) => OrderType::Random,
            Order::Shuffle(_) => OrderType::Shuffle,
        }
    }
}
impl<R: rand::Rng + ?Sized> OrderSource<R> for Order {
    fn next(&mut self, rng: &mut R, weights: Weights<'_>) -> RandResult<usize> {
        match self {
            Order::InOrder(inner) => inner.next(rng, weights),
            Order::Random(inner) => inner.next(rng, weights),
            Order::Shuffle(inner) => inner.next(rng, weights),
        }
    }
}

fn arbitrary_bytes<'a, R: rand::Rng + ?Sized>(
    rng: &mut R,
    buf: &'a mut Vec<u8>,
    count: usize,
) -> RandResult<Unstructured<'a>> {
    buf.resize(count, 0);
    rng.try_fill(&mut buf[..])?;
    Ok(Unstructured::new(buf))
}

#[derive(Clone, Debug, Default)]
pub(super) struct InOrder {
    next_index: usize,
    count: usize,
}
impl<R: rand::Rng + ?Sized> OrderSource<R> for InOrder {
    fn next(&mut self, _rng: &mut R, weights: Weights<'_>) -> RandResult<usize> {
        // PRECONDITION: There exists an index where weights.get_as_usize(index) > 0,
        //               by the definition of `Weights<'_>`
        loop {
            if self.next_index > weights.get_max_index() {
                // wrap index back to beginning
                self.next_index = 0;
                self.count = 0;
            }
            let current = self.next_index;
            let new_count = self.count + 1;

            let goal_weight = weights.get_as_usize(current);
            if self.count >= goal_weight {
                // increment index
                self.next_index = current.wrapping_add(1);
                self.count = 0;
            } else {
                self.count = new_count;
            }
            // count ranges 1..max(weights), so there exists at least one
            // `index` where `count <= weights.get_as_usize(index)`
            if new_count <= goal_weight {
                break Ok(current);
            }
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(super) struct Shuffle {
    prev_items_count: usize,
    indices: Vec<usize>,
}
impl Shuffle {
    fn retain(&mut self, keep_fn: impl Fn(usize) -> bool) {
        let mut search_position = 0;
        while let Some(&value) = self.indices.get(search_position) {
            if keep_fn(value) {
                // valid value (in bounds), continue
                search_position += 1;

                // HOW "closer to the end":
                //   increased search_position
            } else {
                // invalid value, remove
                self.indices.swap_remove(search_position);

                // HOW "closer to the end":
                //   shortened Vec length
                // (no change to search_position, to eval swapped on next iteration)
            }
        }
    }
    fn extend_to_items_count(
        &mut self,
        mut u: Unstructured,
        new_elems: impl Iterator<Item = usize>,
    ) -> arbitrary::Result<()> {
        let prev_len = self.indices.len();

        self.indices.extend(new_elems);
        let new_len = self.indices.len();

        for insert_from in prev_len..new_len {
            let dest = u.choose_index(insert_from + 1)?;
            self.indices.swap(insert_from, dest);
        }

        let remaining = u.take_rest();
        assert!(
            remaining.is_empty(),
            "excess entropy in Unstructured: {remaining:?}"
        );

        Ok(())
    }
}
impl<R: rand::Rng + ?Sized> OrderSource<R> for Shuffle {
    fn next(&mut self, rng: &mut R, weights: Weights<'_>) -> RandResult<usize> {
        // NOTE: Take care to only use 'index' to name the class of return values

        let items_count = weights.get_max_index() + 1;
        if items_count < self.prev_items_count {
            // remove indices that are out of bounds
            self.retain(|value| value < items_count);
        }

        if self.indices.is_empty() {
            // empty `indices` is equivalent to initial filling
            self.prev_items_count = 0;
        }

        if items_count > self.prev_items_count {
            // add new indices
            let new_elems = (self.prev_items_count..items_count)
                .flat_map(|index| std::iter::repeat(index).take(weights.get_as_usize(index)));
            let count = new_elems.clone().count();

            let mut buf = vec![];
            let u = arbitrary_bytes(rng, &mut buf, count.saturating_sub(1))?;
            self.extend_to_items_count(u, new_elems)
                .expect("sufficient Unstructured for extend_to_items_count");
        }
        self.prev_items_count = items_count;

        let popped = self
            .indices
            .pop()
            .expect("should have an element after refilling");
        Ok(popped)
    }
}

#[derive(Clone, Debug, Default)]
pub(super) struct Random {
    // NOTE: Cache is only to reuse allocation, since the effort to
    // validate the cache is similar to just rebuilding from scratch
    choices_buf: Vec<Choice>,
}
#[derive(Clone, Copy, Debug)]
struct Choice {
    index: usize,
    weight_range_max: usize,
}
impl<R: rand::Rng + ?Sized> OrderSource<R> for Random {
    fn next(&mut self, rng: &mut R, weights: Weights<'_>) -> RandResult<usize> {
        let max_index = weights.get_max_index();
        let (breakpoints, max_choice) = if weights.is_unity() {
            (None, max_index)
        } else {
            // calculate breakpoints
            self.choices_buf.clear();

            let mut weight_range_max = 0;
            for index in 0..=max_index {
                let weight = weights.get_as_usize(index);
                if weight == 0 {
                    continue;
                }
                weight_range_max += weight;
                self.choices_buf.push(Choice {
                    index,
                    weight_range_max,
                });
            }

            let max_choice = weight_range_max
                .checked_sub(1)
                .expect("Weights should be non-empty");
            (Some(&self.choices_buf), max_choice)
        };

        let mut buf = vec![];
        let mut u = arbitrary_bytes(rng, &mut buf, 1)?;
        let chosen = u
            .int_in_range(0..=max_choice)
            .expect("sufficient Unstructred for int_in_range");

        if let Some(breakpoints) = breakpoints {
            let choice_index = breakpoints
                .binary_search_by(|c| {
                    use std::cmp::Ordering as O;
                    match c.weight_range_max.cmp(&chosen) {
                        O::Less | O::Equal => O::Less,
                        O::Greater => O::Greater,
                    }
                })
                .unwrap_or_else(|x| x);
            Ok(breakpoints[choice_index].index)
        } else {
            Ok(chosen)
        }
    }
}
