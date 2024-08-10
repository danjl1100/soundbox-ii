// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

//! Ordering for selecting child nodes and child items throughout the [`Network`]

use crate::{child_vec::ChildVec, Child, Network};
use std::rc::Rc;

impl<T, U> Network<T, U> {
    /// Returns a proposed sequence of items leaving the spigot.
    ///
    /// NOTE: Need to finalize the peeked items to progress the [`Network`] state beyond those
    /// peeked items (depending on the child-ordering involved)
    ///
    /// # Errors
    /// Returns any errors reported by the provided [`rand::Rng`] instance
    ///
    /// # Panics
    /// Panics if the internal order state does not match the item node structure
    pub fn peek<'a, R: rand::Rng + ?Sized>(
        &'a self,
        rng: &mut R,
        peek_len: usize,
    ) -> Result<Peeked<'a, T>, rand::Error> {
        let root = &self.root;
        let mut root_order = self.root_order.0.clone();
        let mut root_remaining = CountsRemaining::new(root.len());

        let mut effort_count = 0;

        let chosen_elems = std::iter::from_fn(|| {
            let (elem, effort) = peek_inner(rng, root, &mut root_order, &mut root_remaining);
            effort_count += effort;
            elem
        })
        .take(peek_len)
        .collect();

        Ok(Peeked {
            items: chosen_elems,
            root_order: Root(root_order),
            effort_count,
        })
    }
    /// Finalizes the specified [`Peeked`], advancing the network state (if any)
    pub fn finalize_peeked(&mut self, peeked: PeekAccepted) {
        let PeekAccepted { new_root_order } = peeked;
        self.root_order = new_root_order;
    }
}

fn peek_inner<'a, R, T, U>(
    rng: &mut R,
    current: &'a ChildVec<Child<T, U>>,
    order_node: &mut OrderNode,
    current_remaining: &mut CountsRemaining,
) -> (Option<&'a T>, u64)
where
    R: rand::Rng + ?Sized,
{
    let order_current = &mut order_node.order;
    let order_children = &mut order_node.children;

    let mut effort_count = 0;

    while !current_remaining.is_fully_exhausted() {
        assert_eq!(current.len(), order_children.len());
        assert_eq!(current.len(), current_remaining.child_count_if_nonempty());

        let child_index = order_current
            .next_in(rng, current)
            .expect("current should not be empty");

        let remaining_slot = current_remaining.child_mut(child_index);
        if remaining_slot.is_none() {
            // chosen child is known to to be exhausted
            continue;
        }

        #[allow(clippy::panic)]
        let Some(child_node) = current.children().get(child_index) else {
            panic!("valid current.children index ({child_index}) from order")
        };
        #[allow(clippy::panic)]
        let Some(child_order) = order_children.get_mut(child_index) else {
            panic!("valid order_children index ({child_index}) from order")
        };

        // effort: lookup child_node and child_order
        effort_count += 1;

        let elem = match child_node {
            Child::Bucket(bucket) => {
                let bucket_items = &bucket.items;
                if bucket_items.is_empty() {
                    None
                } else {
                    let elem_index = Rc::make_mut(child_order)
                        .order
                        .next_in_equal(rng, bucket_items)
                        .expect("bucket should not be empty");
                    #[allow(clippy::panic)]
                    let Some(elem) = bucket_items.get(elem_index) else {
                        panic!("valid bucket_items index ({elem_index}) from order")
                    };

                    // effort: lookup bucket element
                    effort_count += 1;

                    Some(elem)
                }
            }
            Child::Joint(joint) => {
                if joint.next.is_empty() {
                    None
                } else if let Some(remaining) = remaining_slot {
                    let (elem, child_effort_count) = peek_inner(
                        rng,
                        &joint.next,
                        Rc::make_mut(child_order),
                        remaining.as_mut_or_init(|| CountsRemaining::new(joint.next.len())),
                    );

                    // effort: recursion effort
                    effort_count += child_effort_count;

                    elem
                } else {
                    None
                }
            }
        };
        if let Some(elem) = elem {
            return (Some(elem), effort_count);
        }
        current_remaining.set_empty(child_index);
    }
    (None, effort_count)
}

/// Resulting items and tentative ordering state from [`Network::peek`]
pub struct Peeked<'a, T> {
    // TODO include metadata for which node the item came from
    items: Vec<&'a T>,
    root_order: Root,
    effort_count: u64,
}
impl<'a, T> Peeked<'a, T> {
    /// Returns an the peeked items
    #[must_use]
    pub fn items(&self) -> &[&'a T] {
        &self.items
    }
    /// Cancels the peek operation and returns the referenced items
    #[must_use]
    pub fn cancel_into_items(self) -> Vec<&'a T> {
        self.items
    }
    /// Accepts the peeked items, discarding them to allow updating the original network
    pub fn accept_into_inner(self) -> PeekAccepted {
        PeekAccepted {
            new_root_order: self.root_order,
        }
    }
    #[allow(unused)]
    /// For tests only, return the amount of effort required for this peek result
    pub(crate) fn get_effort_count(&self) -> u64 {
        self.effort_count
    }
}
/// Resulting tentative ordering state from [`Network::peek`] to apply in
/// [`Network::finalize_peeked`]
#[must_use]
pub struct PeekAccepted {
    new_root_order: Root,
}

// `Option<Lazy<T>>` seems cleaner and more meaningful than `Option<Option<T>>`
// (heeding advice from the pedantic lint `clippy::option_option`)
#[derive(Clone, Default)]
enum Lazy<T> {
    Value(T),
    #[default]
    Uninit,
}
impl<T> Lazy<T> {
    fn as_mut(&mut self) -> Option<&mut T> {
        match self {
            Self::Value(value) => Some(value),
            Self::Uninit => None,
        }
    }
    fn as_mut_or_init(&mut self, init_fn: impl FnOnce() -> T) -> &mut T {
        match self {
            Self::Value(_) => {}
            Self::Uninit => {
                *self = Self::Value(init_fn());
            }
        }
        self.as_mut().expect("should initialize directly above")
    }
}

#[derive(Clone)]
struct CountsRemaining(Vec<Option<Lazy<Self>>>);
impl CountsRemaining {
    fn new(len: usize) -> Self {
        Self(vec![Some(Lazy::default()); len])
    }
    /// # Panics
    /// Panics if the index is out of bounds (greater than `len` provided in [`Self::new`]),
    /// or all children are exhausted.
    fn set_empty(&mut self, index: usize) {
        self.0[index].take();

        // check if all are exhausted
        if self.0.iter().all(Option::is_none) {
            // ensure any future calls error (loudly)
            self.0.clear();
        }
    }
    /// Returns a mutable reference to the child's remaining count (which may not yet be
    /// initialized) or `None` if the child is exhausted (e.g. via [`Self::set_empty`])
    ///
    /// # Panics
    /// Panics if the index is out of bounds (greater than `len` provided in [`Self::new`]),
    /// or all children are exhausted.
    fn child_mut(&mut self, index: usize) -> Option<&mut Lazy<Self>> {
        self.0[index].as_mut()
    }
    /// Returns true if all children are exhausted
    fn is_fully_exhausted(&self) -> bool {
        self.0.is_empty()
    }
    /// Returns the number of children, or `0` if all children are exhausted
    fn child_count_if_nonempty(&self) -> usize {
        self.0.len()
    }
}

trait OrderSource<R: rand::Rng + ?Sized> {
    /// Returns the next index in the order, within the range `0..=max_index`
    fn next(&mut self, rng: &mut R, weights: Weights<'_>) -> usize;
    /// Returns the next index in the order to index the specified target slice
    /// or `None` if the specified `target` is empty.
    fn next_in_equal<T>(&mut self, rng: &mut R, target: &[T]) -> Option<usize> {
        let weights = Weights::new_equal(target.len())?;
        let next = self.next(rng, weights);
        Some(next)
    }
    /// Returns the next index in the order to index the specified target [`ChildVec`],
    /// or `None` if the specified `target` is empty.
    fn next_in<T>(&mut self, rng: &mut R, target: &ChildVec<T>) -> Option<usize> {
        let weights = if target.weights().is_empty() {
            // returns `None` if length is zero
            Weights::new_equal(target.len())?
        } else {
            // returns `None` if weights are all zero
            Weights::new_custom(target.weights())?
        };
        let next = self.next(rng, weights);
        Some(next)
    }
}

use weights::Weights;
mod weights {
    /// Non-empty weights (length non-zero, and contents non-zero)
    #[derive(Clone, Copy, Debug)]
    pub(super) struct Weights<'a>(Inner<'a>);

    #[derive(Clone, Copy, Debug)]
    enum Inner<'a> {
        Unity {
            /// NOTE: specifically chosen to avoid awkward `len = 0` case
            /// e.g. there must be a `next` available for unsigned `max_index >= 0`
            max_index: usize,
        },
        Custom {
            /// Non-empty weights (e.g. at least one nonzero element)
            weights: &'a [u32],
        },
    }
    impl<'a> Weights<'a> {
        /// Returns `None` if the specified slice is empty
        pub fn new_custom(weights: &'a [u32]) -> Option<Self> {
            if weights.is_empty() {
                None
            } else {
                assert!(!weights.is_empty());
                weights
                    .iter()
                    .any(|&w| w != 0)
                    .then_some(Self(Inner::Custom { weights }))
            }
        }
        pub fn new_equal(len: usize) -> Option<Self> {
            let max_index = len.checked_sub(1)?;
            Some(Self(Inner::Unity { max_index }))
        }
        pub fn get_max_index(self) -> usize {
            let Self(inner) = self;
            match inner {
                Inner::Unity { max_index } => max_index,
                Inner::Custom { weights } => weights.len() - 1,
            }
        }
        pub fn get(self, index: usize) -> u32 {
            let Self(inner) = self;
            match inner {
                Inner::Unity { max_index } => {
                    assert!(
                        index <= max_index,
                        "index should be in bounds for Weights::get"
                    );
                    1
                }
                Inner::Custom { weights } => weights[index],
            }
        }
        pub fn get_as_usize(self, index: usize) -> usize {
            self.get(index)
                .try_into()
                .expect("weight should fit in platform's usize")
        }
    }
}

use node::Node as OrderNode;
pub(crate) use node::{Root, UnknownOrderPath};
mod node {
    //! Tree structure for [`Order`], meant to mirror the
    //! [`Network`](`crate::Network`) topology.

    use super::Order;
    use crate::path::{Path, PathRef};
    use std::rc::Rc;

    #[derive(Clone, Default, Debug)]
    pub(crate) struct Root(pub(super) Node);
    #[derive(Clone, Debug, Default)]
    pub struct Node {
        pub(super) order: Order,
        pub(super) children: Vec<Rc<Node>>,
    }

    impl Root {
        /// Adds a default node at the specified path.
        ///
        /// Returns the index of the new child on success.
        pub(crate) fn add(&mut self, path: PathRef<'_>) -> Result<usize, UnknownOrderPath> {
            let mut current_children = &mut self.0.children;

            for next_index in path {
                let Some(next_child) = current_children.get_mut(next_index) else {
                    return Err(UnknownOrderPath(path.clone_inner()));
                };
                current_children = &mut Rc::make_mut(next_child).children;
            }

            let new_index = current_children.len();

            current_children.push(Rc::new(Node::default()));

            Ok(new_index)
        }
    }

    /// The specified path does not match an order-node
    #[derive(Debug)]
    pub struct UnknownOrderPath(pub(crate) Path);
}

#[derive(Clone, Copy, Debug, Default)]
enum OrderType {
    #[default]
    InOrder,
}
impl OrderType {
    #[cfg(test)]
    const ALL: &'static [Self] = &[
        // rustfmt, delete when multiple items are present
        Self::InOrder,
    ];
}
impl std::fmt::Display for OrderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrderType::InOrder => write!(f, "InOrder"),
        }
    }
}

#[derive(Clone, Debug)]
enum Order {
    InOrder(InOrder),
}
impl Default for Order {
    fn default() -> Self {
        Self::new(OrderType::default())
    }
}
impl Order {
    fn new(ty: OrderType) -> Self {
        match ty {
            OrderType::InOrder => Self::InOrder(InOrder::default()),
        }
    }
    // TODO remove if unused Order::get_ty
    // fn get_ty(&self) -> OrderType {
    //     match self {
    //         Order::InOrder(_) => OrderType::InOrder,
    //     }
    // }
}
impl<R: rand::Rng + ?Sized> OrderSource<R> for Order {
    fn next(&mut self, rng: &mut R, weights: Weights<'_>) -> usize {
        match self {
            Order::InOrder(inner) => inner.next(rng, weights),
        }
    }
}

#[derive(Clone, Debug, Default)]
struct InOrder {
    next_index: usize,
    count: usize,
}
impl<R: rand::Rng + ?Sized> OrderSource<R> for InOrder {
    fn next(&mut self, _rng: &mut R, weights: Weights<'_>) -> usize {
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
                break current;
            }
        }
    }
}

// TODO add Shuffle (randomize the order of items, then proceed with that fixed order)
//      ^^ for Shuffle, test (could) assert that all items are visited (for sufficient `next`s)
//      ^^ for Shuffle, test can assert that (initially) there are no repeat items, until all seen
// TODO add Random (selection of the next item is random, independent from prior selections)
//      ^^ for Random, main assertion is that weights vaguely affect the outcome (with huge margin, stats are weird)

#[cfg(test)]
mod tests {
    #![allow(clippy::panic)]

    use super::*;
    use crate::tests::{fake_rng, run_with_timeout};
    use arbtest::arbitrary::Unstructured;
    use std::{cell::Cell, time::Duration};

    const NONEMPTY_WEIGHTS: &str = "weights should be nonempty";

    // per https://github.com/rust-lang/rust/issues/92698#issuecomment-1680155957
    macro_rules! let_workaround {
        (let $name:ident = $val:expr; $($rest:tt)+) => {
            match $val {
                $name => {
                    let_workaround! { $($rest)+ }
                }
            }
        };
        ($($rest:tt)+) => { $($rest)+ }
    }

    /// Wraps the specified `next_fn` closure, validating the results according to the
    /// [`OrderType`]
    fn validate_next<'a>(
        ty: OrderType,
        weights: Weights<'a>,
        counter: Option<&'a Cell<u64>>,
        mut next_fn: impl FnMut(&mut Unstructured) -> usize + 'a + Send + Sync,
    ) -> impl FnMut(&mut Unstructured) -> arbtest::arbitrary::Result<std::ops::ControlFlow<()>> + 'a
    {
        const TIMEOUT: Duration = Duration::from_secs(1);

        let mut prev = None;
        move |u| {
            if let Some(counter) = counter {
                counter.replace(counter.get() + 1);
            }
            let next = run_with_timeout(
                || next_fn(u),
                TIMEOUT,
                |elapsed| {
                    // FIXME no way of reporting a "failure" seed if `next_fn` is stuck,
                    //       since only the process abort will cancel the function
                    eprintln!("aborting process, call to `next` (type {ty}) took longer than {elapsed:?}\nEXIT 1");
                    std::process::exit(1)
                },
            );
            match (ty, prev) {
                (OrderType::InOrder, None) => {}
                (OrderType::InOrder, Some(prev)) => validate_next_in_order(prev, next, weights),
            }
            prev = Some(next);
            Ok(std::ops::ControlFlow::Continue(()))
        }
    }

    /// Identifies clear violtaions in the sequential output from [`InOrder`]
    fn validate_next_in_order(prev: usize, next: usize, weights: Weights<'_>) {
        let prev_plus_one = prev + 1;
        let max_index = weights.get_max_index();

        // same, completing the count
        if next == prev {
            return;
        }
        // step up
        if next == prev + 1 {
            return;
        }
        // wrap around to 0
        if next == 0 && prev == weights.get_max_index() {
            return;
        }

        // increased, skipping zero-weight entries
        let idx_to_check = || prev_plus_one..next;
        if prev_plus_one < next && idx_to_check().map(|i| weights.get(i)).all(|w| w == 0) {
            let checked_count = idx_to_check().count();
            let_workaround! {
                let idx_fmt = format_args!("{prev_plus_one}..{next}");
                assert!(
                    checked_count > 0,
                    "should check `all` on nonempty iter {idx_fmt}"
                );
                // println!(
                //     "VALID prev {prev} -> next {next}, see zero weights {idx_fmt}",
                // );
            }
            return;
        }

        // wrapped around, skipping zero-weight entries
        let idx_to_check = || ((prev + 1)..=max_index).chain(0..next);
        if idx_to_check().map(|i| weights.get(i)).all(|w| w == 0) {
            let checked_count = idx_to_check().count();
            let_workaround! {
                let idx_fmt = format_args!("{prev_plus_one}..={max_index} and 0..{next}");
                assert!(
                    checked_count > 0,
                    "should check `all` on nonempty iter {idx_fmt}"
                );
                // println!(
                //     "VALID prev {prev} -> next {next}, see zero weights {idx_fmt}",
                // );
            }
            return;
        }

        panic!("prev {prev} -> next {next} should be a sane step")
    }

    /// Exhaustively test [`Order`] for all [`OrderType`]s using [`arbtest`], first for various `len` then various `weights`
    ///
    /// Basic assertion: always terminates for fixed number of polling
    #[test]
    fn arb_weights() {
        for &ty in OrderType::ALL {
            let mut uut = Order::new(ty);

            // phase 1 of test - equal weights
            arbtest::arbtest(|u| {
                let equal_len_u32: u32 = u.int_in_range(1..=1_000)?;
                let equal_len: usize = equal_len_u32.try_into().expect("u32 should fit in usize");
                let weights = Weights::new_equal(equal_len).expect("test len should be nonzero");

                let counter = Cell::default();
                u.arbitrary_loop(
                    Some(1),
                    Some(equal_len_u32 * 10),
                    validate_next(ty, weights, Some(&counter), |u| {
                        uut.next(&mut fake_rng(u), weights)
                    }),
                )?;
                assert!(
                    counter.into_inner() > 0,
                    "should run some iterations for equal weights {weights:?}"
                );
                Ok(())
            });

            // phase 2 of test - custom weights
            arbtest::arbtest(|u| {
                let weight_values: Vec<u8> = u.arbitrary()?;
                let weights: Vec<u32> = weight_values.into_iter().map(u32::from).collect();
                let weights_sum: u32 = weights.iter().sum();

                if weights.iter().all(|&x| x == 0) {
                    return Ok(());
                }
                let weights =
                    Weights::new_custom(&weights).expect("test weights should be nonempty");

                let counter = Cell::default();
                u.arbitrary_loop(
                    Some(1),
                    Some(weights_sum * 2),
                    validate_next(ty, weights, Some(&counter), |u| {
                        uut.next(&mut fake_rng(u), weights)
                    }),
                )?;
                assert!(
                    counter.into_inner() > 0,
                    "should run some iterations for custom weights {weights:?}"
                );
                Ok(())
            });
        }
    }

    #[test]
    fn in_order_equal() {
        let mut uut = InOrder::default();
        let rng = &mut crate::tests::PanicRng;
        let mut next = |max_index| {
            uut.next(
                rng,
                Weights::new_equal(max_index + 1usize).expect("usize + 1 should be nonzero"),
            )
        };

        assert_eq!(next(5), 0);
        assert_eq!(next(5), 1);
        assert_eq!(next(5), 2);
        assert_eq!(next(5), 3);
        assert_eq!(next(5), 4);
        assert_eq!(next(5), 5);
        //
        assert_eq!(next(5), 0);
        //
        assert_eq!(next(2), 1);
        assert_eq!(next(2), 2);
        assert_eq!(next(2), 0);
        assert_eq!(next(2), 1);
        //
        assert_eq!(next(1), 0);
        //
        assert_eq!(next(0), 0);
        assert_eq!(next(0), 0);
    }
    #[test]
    fn in_order_decrease_weights() {
        let rng = &mut crate::tests::PanicRng;

        let weights = &[3, 1];
        let weights = Weights::new_custom(weights).expect(NONEMPTY_WEIGHTS);
        let weights_reduced = &[2, 1];
        let weights_reduced = Weights::new_custom(weights_reduced).expect(NONEMPTY_WEIGHTS);

        let mut uut = InOrder::default();
        assert_eq!(uut.next(rng, weights), 0);
        assert_eq!(uut.next(rng, weights), 0);
        assert_eq!(uut.next(rng, weights), 0);

        let mut uut = InOrder::default();
        assert_eq!(uut.next(rng, weights), 0);
        assert_eq!(uut.next(rng, weights), 0);
        assert_eq!(uut.next(rng, weights_reduced), 1);
    }
}
