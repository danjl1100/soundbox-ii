// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use self::need_run::NeedRun;
use crate::FakeVlc;

/// Created by [`arbtest_with_fake_vlc`]
#[must_use = "call `run()` to run the test"]
pub struct ArbTestWithFakeVlc<F> {
    test_fn: F,
    budget_ms: Option<u64>,
    seed: Option<u64>,
    hint_need_run: NeedRun,
}
/// Ergonomic wrapper combining `with_fake_vlc` and `arbtest`.
pub fn arbtest_with_fake_vlc<F>(test_fn: F) -> ArbTestWithFakeVlc<F>
where
    F: FnMut(
        &mut arbtest::arbitrary::Unstructured<'_>,
        &FakeVlc,
        &mut vlc_http_ureq::HttpRunner,
    ) -> arbtest::arbitrary::Result<()>,
{
    ArbTestWithFakeVlc {
        test_fn,
        budget_ms: None,
        seed: None,
        hint_need_run: NeedRun::default(),
    }
}
#[expect(missing_docs, reason = "matches ArbTest function purposes")]
impl<F> ArbTestWithFakeVlc<F>
where
    F: FnMut(
        &mut arbtest::arbitrary::Unstructured<'_>,
        &FakeVlc,
        &mut vlc_http_ureq::HttpRunner,
    ) -> arbtest::arbitrary::Result<()>,
{
    pub fn budget_ms(mut self, ms: u64) -> Self {
        self.budget_ms = Some(ms);
        self
    }
    pub fn seed(mut self, seed: u64) -> Self {
        self.seed = Some(seed);
        self
    }
    /// # Errors
    ///
    /// Returns an error if [`FakeVlc::new()`] fails
    pub fn run(mut self) -> eyre::Result<()> {
        self.hint_need_run.defuse();

        FakeVlc::with_new(|vlc, runner| {
            let arb = arbtest::arbtest(|u| (self.test_fn)(u, vlc, runner));
            let arb = if let Some(ms) = self.budget_ms {
                arb.budget_ms(ms)
            } else {
                arb
            };
            let arb = if let Some(seed) = self.seed {
                arb.seed(seed)
            } else {
                arb
            };
            arb.run();
            Ok(())
        })
    }
}

mod need_run {
    struct Defused {}

    #[derive(Default)]
    pub struct NeedRun(Option<Defused>);
    impl NeedRun {
        pub fn defuse(&mut self) {
            let Self(inner) = self;
            *inner = Some(Defused {});
        }
    }
    impl Drop for NeedRun {
        fn drop(&mut self) {
            let Self(inner) = self;
            if std::thread::panicking() {
                // only bother if we're not already panicking
                return;
            }
            #[expect(clippy::panic, reason = "detect no-op test usage")]
            let Some(Defused {}) = inner else {
                panic!("ArbTestWithFakeVlc dropped without a call to `.run()`")
            };
        }
    }
}
