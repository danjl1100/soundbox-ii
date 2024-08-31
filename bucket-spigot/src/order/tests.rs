// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

#![allow(clippy::panic)]

use super::source::{InOrder, Order, OrderSource as _, OrderType, Random};
use super::RandResult;
use crate::tests::{assert_arb_error, decode_hex, fake_rng, run_with_timeout};
use crate::Weights;
use arbtest::arbitrary::Unstructured;
use std::time::Duration;

const NONEMPTY_WEIGHTS: &str = "weights should be nonempty";

fn env_arbtest<P>(predicate: P) -> arbtest::ArbTest<P>
where
    P: FnMut(&mut Unstructured) -> arbitrary::Result<()>,
{
    let test = arbtest::arbtest(predicate);
    if std::env::var("ARBTEST_LONG").is_ok() {
        println!("running full 10 seconds (ARBTEST_LONG)");
        test.budget_ms(10_000)
    } else {
        test
    }
}

// per https://users.rust-lang.org/t/rpitit-allows-more-flexible-code-in-comparison-with-raw-rpit-in-inherit-impl/113417/2
// usage:
//     // To imply 'a: 'b, express as a reference
//     fn f<'a, 'b>(&'a self, ......) -> impl Trait + Captures<&'b &'a ()>
trait Captures<T: ?Sized> {}
impl<T: ?Sized, U: ?Sized> Captures<T> for U {}

struct Validator<'a> {
    step_count: usize,
    seen: Vec<u32>,
    order_type: OrderType,
    weights: Weights<'a>,
    weights_sum: usize,
    uut_changed_weights: bool,
}

impl<'a> Validator<'a> {
    fn new(order_type: OrderType, weights: Weights<'a>, uut_changed_weights: bool) -> Self {
        let weights_sum: usize = (0..=weights.get_max_index())
            .map(|index| weights.get_as_usize(index))
            .sum();
        Self {
            step_count: 0,
            seen: vec![],
            order_type,
            weights,
            weights_sum,
            uut_changed_weights,
        }
    }
    /// Wraps the specified `next_fn` closure, validating the results according to the
    /// [`OrderType`]
    fn validate_next<'b>(
        &'b mut self,
        mut next_fn: impl FnMut(&mut Unstructured) -> RandResult<usize> + Send + Sync + 'b,
        // TODO type alias for arbtest::arbitrary::Result<ControlFlow<()>>
    ) -> impl FnMut(&mut Unstructured) -> arbtest::arbitrary::Result<std::ops::ControlFlow<()>>
           + Captures<&'a &'b ()> {
        const TIMEOUT: Duration = Duration::from_secs(1);

        let mut prev = None;
        move |u| {
            let next = run_with_timeout(
                || assert_arb_error(next_fn(u)),
                TIMEOUT,
                |elapsed| {
                    // FIXME no way of reporting a "failure" seed if `next_fn` is stuck,
                    //       since only the process abort will cancel the function
                    eprintln!("aborting process, call to `next` (type {ty}) took longer than {elapsed:?}\nEXIT 1", ty=self.order_type);
                    std::process::exit(1)
                },
            )?;
            match (self.order_type, prev) {
                (OrderType::InOrder, None) => {}
                (OrderType::InOrder, Some(prev)) => {
                    self.validate_next_in_order(prev, next);
                }
                (OrderType::Random, _) => {
                    self.validate_next_random(next);
                }
                (OrderType::Shuffle, _) => {
                    self.validate_next_shuffle(next);
                }
            }
            prev = Some(next);

            let max_index = self.weights.get_max_index();
            assert!(
                next <= max_index,
                "next {next} should be within max_index {max_index}"
            );
            if max_index >= self.seen.len() {
                self.seen.resize(max_index + 1, 0);
            }
            self.seen[next] += 1;

            self.step_count += 1;

            if !self.uut_changed_weights && self.step_count == self.weights_sum {
                match self.order_type {
                    OrderType::InOrder | OrderType::Random => {}
                    OrderType::Shuffle => self.validate_end_shuffle(),
                }
            }

            Ok(std::ops::ControlFlow::Continue(()))
        }
    }

    fn into_step_count(self) -> usize {
        self.step_count
    }

    /// Identifies clear violtaions in the sequential output from [`InOrder`]
    fn validate_next_in_order(&self, prev: usize, next: usize) {
        let prev_plus_one = prev + 1;
        let max_index = self.weights.get_max_index();

        // same, completing the count
        if next == prev {
            return;
        }
        // step up
        if next == prev + 1 {
            return;
        }
        // wrap around to 0
        if next == 0 && prev == self.weights.get_max_index() {
            return;
        }

        // increased, skipping zero-weight entries
        let idx_to_check = || prev_plus_one..next;
        if prev_plus_one < next && idx_to_check().map(|i| self.weights.get(i)).all(|w| w == 0) {
            let checked_count = idx_to_check().count();
            assert!(
                checked_count > 0,
                "should check `all` on nonempty iter {prev_plus_one}..{next}"
            );
            return;
        }

        // wrapped around, skipping zero-weight entries
        let idx_to_check = || ((prev + 1)..=max_index).chain(0..next);
        if idx_to_check().map(|i| self.weights.get(i)).all(|w| w == 0) {
            let checked_count = idx_to_check().count();
            assert!(
                checked_count > 0,
                "should check `all` on nonempty iter {prev_plus_one}..={max_index} and 0..{next}"
            );
            return;
        }

        panic!("prev {prev} -> next {next} should be a sane step")
    }

    fn validate_next_random(&self, next: usize) {
        let weight = self.weights.get(next);
        assert!(
            weight != 0,
            "should not select {next}, which has weight {weight}"
        );
    }

    fn validate_next_shuffle(&self, next: usize) {
        let len = self.weights.get_max_index() + 1;
        if (1..len).contains(&self.step_count) && !self.uut_changed_weights {
            let seen = self.seen[next];
            let target_weight = self.weights.get(next);
            assert!(
                seen < target_weight,
                "already seen: {next} (seen {seen} >= target_weight {target_weight})"
            );
        }
    }

    fn validate_end_shuffle(&self) {
        if !self.uut_changed_weights {
            let ratios: Vec<_> = self
                .seen
                .iter()
                .enumerate()
                .map(|(index, &seen)| {
                    let weight = self.weights.get(index);
                    if weight == 0 {
                        Err(seen)
                    } else {
                        Ok(f64::from(seen) / f64::from(weight))
                    }
                })
                .collect();
            let first = ratios
                .iter()
                .copied()
                .find_map(Result::ok)
                .expect("seen should be nonempty");
            for (index, ratio) in ratios.into_iter().enumerate() {
                match ratio {
                    Ok(ratio) => {
                        let relative_to_first = ratio / first;
                        assert!(
                        relative_to_first > 0.9 && relative_to_first < 1.1,
                        "{index}: {ratio} ratios should be similar, first {first}, relative_to_first {relative_to_first}"
                    );
                    }
                    Err(seen) => {
                        assert_eq!(
                            seen, 0,
                            "{index}: {seen} seen should be zero for zero weight"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn arb_weights_equal_in_order() {
    arb_weights_equal(OrderType::InOrder);
}
#[test]
fn arb_weights_equal_random() {
    arb_weights_equal(OrderType::Random);
}
#[test]
fn arb_weights_equal_shuffle() {
    arb_weights_equal(OrderType::Shuffle);
}
/// Exhaustively test [`Order`] for all [`OrderType`]s using [`arbtest`], first for various `len`
///
/// Basic assertion: always terminates for fixed number of polling
fn arb_weights_equal(
    ty: OrderType,
) -> arbtest::ArbTest<impl FnMut(&mut Unstructured) -> arbitrary::Result<()>> {
    // phase 1 of test - equal weights
    env_arbtest(move |u| {
        let mut uut = Order::new(ty);

        // Repeat, to bridge UUT over several weights
        for i in 0..2 {
            // let equal_len_u32: u32 = u.int_in_range(1..=1_000)?;
            // let equal_len: usize = equal_len_u32.try_into().expect("u32 should fit in usize");
            let equal_len = u.arbitrary_len::<u32>()?;
            let equal_len_u32: u32 = equal_len
                .try_into()
                .expect("usize for remaining entropy should fit in u32");

            if equal_len == 0 {
                return Ok(());
            }

            let weights = Weights::new_equal(equal_len).expect("test len should be nonzero");

            let uut_changed_weights = i > 0;
            let mut validator = Validator::new(ty, weights, uut_changed_weights);

            u.arbitrary_loop(
                Some(1),
                Some(equal_len_u32 * 10),
                validator.validate_next(|u| uut.next(&mut fake_rng(u), weights)),
            )?;
            let step_count = validator.into_step_count();
            assert!(
                step_count > 0,
                "should run some iterations for equal weights {weights:?}"
            );
        }
        Ok(())
    })
}

#[test]
fn arb_weights_custom_in_order() {
    arb_weights_custom(OrderType::InOrder);
}
#[test]
fn arb_weights_custom_random() {
    arb_weights_custom(OrderType::Random);
}
#[test]
fn arb_weights_custom_shuffle() {
    arb_weights_custom(OrderType::Shuffle);
}
/// Exhaustively test [`Order`] for all [`OrderType`]s using [`arbtest`], first for various `weights`
///
/// Basic assertion: always terminates for fixed number of polling
fn arb_weights_custom(
    ty: OrderType,
) -> arbtest::ArbTest<impl FnMut(&mut Unstructured) -> arbitrary::Result<()>> {
    // phase 2 of test - custom weights
    env_arbtest(move |u| {
        let mut uut = Order::new(ty);

        // Repeat, to bridge UUT over several weights
        for i in 0..2 {
            let weight_values: Vec<u8> = u.arbitrary()?;
            let weights: Vec<u32> = weight_values.into_iter().map(u32::from).collect();
            let weights_sum: u32 = weights.iter().sum();

            if weights.iter().all(|&x| x == 0) {
                return Ok(());
            }
            let weights = Weights::new_custom(&weights).expect("test weights should be nonempty");

            let uut_changed_weights = i > 0;
            let mut validator = Validator::new(ty, weights, uut_changed_weights);

            u.arbitrary_loop(
                Some(1),
                Some(weights_sum * 2),
                validator.validate_next(|u| uut.next(&mut fake_rng(u), weights)),
            )?;
            let step_count = validator.into_step_count();
            assert!(
                step_count > 0,
                "should run some iterations for custom weights {weights:?}"
            );
        }
        Ok(())
    })
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
        .expect("should not rand::Error")
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
#[allow(clippy::unwrap_used)]
fn in_order_decrease_weights() {
    let rng = &mut crate::tests::PanicRng;

    let weights = &[3, 1];
    let weights = Weights::new_custom(weights).expect(NONEMPTY_WEIGHTS);
    let weights_reduced = &[2, 1];
    let weights_reduced = Weights::new_custom(weights_reduced).expect(NONEMPTY_WEIGHTS);

    let mut uut = InOrder::default();
    assert_eq!(uut.next(rng, weights).unwrap(), 0);
    assert_eq!(uut.next(rng, weights).unwrap(), 0);
    assert_eq!(uut.next(rng, weights).unwrap(), 0);

    let mut uut = InOrder::default();
    assert_eq!(uut.next(rng, weights).unwrap(), 0);
    assert_eq!(uut.next(rng, weights).unwrap(), 0);
    assert_eq!(uut.next(rng, weights_reduced).unwrap(), 1);
}

#[test]
fn random_looks_decent() {
    fn chunks<R: rand::Rng + ?Sized>(uut: &mut Random, weights: &[u32], rng: &mut R) -> String {
        use std::fmt::Write as _;
        let weights = Weights::new_custom(weights).expect(NONEMPTY_WEIGHTS);
        let mut iter = std::iter::repeat_with(|| uut.next(rng, weights).expect("should not fail"));

        // NOTE: assert_debug_snapshot is too verbose (line endings from `:#?`)
        let mut output = String::new();
        for _line in 0..5 {
            for elem in 0..30 {
                let space = if elem == 0 { "" } else { " " };
                write!(&mut output, "{space}{}", iter.next().expect("infinite"))
                    .expect("infallible");
            }
            writeln!(&mut output).expect("infallible");
        }
        output
    }

    let determined = decode_hex(&[
        // chosen by fair dice roll, then paintsakingly trimmed to length
        // (`head --bytes=? /dev/urandom | sha256sum`)
        "650ef28d459cd670558cc769820c0e6b09d41388667068cb0c73390604149f68",
        "568d8549e361fe905d928bf3d0f862c046f5c4cdb5cad04f8ef8b956341483e6",
        "f2d88bf0fcf77f2a2f56ed81c94479e395133348f3958374050b9cc06eb5b129",
        "aa5b814f53abe186cfacbbfdabe2f90ab8c071dc4ed50dcf1d4362a46f0e6348",
        "0ada9fbdc9962e02271c7f93fa2cbe5389cdebf13e8f",
    ])
    .expect("valid hex strings");

    let weights = &[10, 2, 1]; // 0 - 2

    let mut uut = Random::default();

    let mut u = Unstructured::new(&determined);
    let rng = &mut crate::tests::fake_rng(&mut u);
    insta::assert_snapshot!(chunks(&mut uut, weights, rng), @r###"
    1 0 0 1 0 0 0 0 0 1 0 0 0 2 0 0 0 0 0 0 1 0 0 0 2 1 0 0 0 0
    0 0 0 1 0 0 0 0 0 0 0 0 0 0 0 0 0 1 0 1 0 1 2 0 0 0 2 0 0 0
    0 0 0 0 0 0 0 0 0 0 1 0 0 0 0 2 0 0 0 0 0 0 2 0 0 0 0 2 0 1
    0 1 0 2 0 0 0 0 2 0 0 0 0 0 2 0 0 0 0 0 0 1 0 1 0 2 0 0 0 2
    0 0 0 0 0 0 0 0 1 1 0 0 0 0 0 0 0 0 1 0 0 0 0 0 0 1 0 0 1 0
    "###);
    assert_eq!(u.len(), 0);

    let weights = &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10, 2, 1]; // 10-12

    let mut u = Unstructured::new(&determined);
    let rng = &mut crate::tests::fake_rng(&mut u);
    insta::assert_snapshot!(chunks(&mut uut, weights, rng), @r###"
    11 10 10 11 10 10 10 10 10 11 10 10 10 12 10 10 10 10 10 10 11 10 10 10 12 11 10 10 10 10
    10 10 10 11 10 10 10 10 10 10 10 10 10 10 10 10 10 11 10 11 10 11 12 10 10 10 12 10 10 10
    10 10 10 10 10 10 10 10 10 10 11 10 10 10 10 12 10 10 10 10 10 10 12 10 10 10 10 12 10 11
    10 11 10 12 10 10 10 10 12 10 10 10 10 10 12 10 10 10 10 10 10 11 10 11 10 12 10 10 10 12
    10 10 10 10 10 10 10 10 11 11 10 10 10 10 10 10 10 10 11 10 10 10 10 10 10 11 10 10 11 10
    "###);
    assert_eq!(u.len(), 0);
}
