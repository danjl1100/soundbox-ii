// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

/// Random Number Generator that is fed by a deterministic `arbtest::arbitrary`
pub(crate) struct ArbitraryRng<'a, 'b>(&'a mut arbtest::arbitrary::Unstructured<'b>)
where
    'b: 'a;
impl<'a, 'b> rand::RngCore for ArbitraryRng<'a, 'b> {
    fn next_u32(&mut self) -> u32 {
        unreachable!("non-fallible RngCore method called");
    }
    fn next_u64(&mut self) -> u64 {
        unreachable!("non-fallible RngCore method called");
    }
    fn fill_bytes(&mut self, _dest: &mut [u8]) {
        unreachable!("non-fallible RngCore method called");
    }
    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand::Error> {
        for dest in dest {
            if self.0.is_empty() {
                // notify the test harness that entropy is empty
                return Err(rand::Error::new(arbtest::arbitrary::Error::NotEnoughData));
            }
            *dest = self.0.arbitrary().map_err(rand::Error::new)?;
        }
        Ok(())
    }
}
pub(crate) fn fake_rng<'a, 'b>(
    arbitrary: &'a mut arbtest::arbitrary::Unstructured<'b>,
) -> ArbitraryRng<'a, 'b> {
    ArbitraryRng(arbitrary)
}

/// Rng that panics when called
pub(crate) struct PanicRng;
impl rand::RngCore for PanicRng {
    fn next_u32(&mut self) -> u32 {
        unreachable!("next_u32 in PanicRng");
    }
    fn next_u64(&mut self) -> u64 {
        unreachable!("next_u64 in PanicRng");
    }
    fn fill_bytes(&mut self, _dest: &mut [u8]) {
        unreachable!("fill_bytes in PanicRng");
    }
    fn try_fill_bytes(&mut self, _dest: &mut [u8]) -> Result<(), rand::Error> {
        unreachable!("try_fill_bytes in PanicRng");
    }
}

fn extract_arb_error<T>(
    result: Result<T, rand::Error>,
) -> Result<Result<T, arbtest::arbitrary::Error>, Box<dyn std::error::Error + Sync + Send>> {
    match result {
        Ok(value) => Ok(Ok(value)),
        Err(err) => {
            let inner_error = err.take_inner();
            match inner_error.downcast() {
                Ok(arb_error) => Ok(Err(*arb_error)),
                Err(other_error) => Err(other_error),
            }
        }
    }
}
pub(crate) fn assert_arb_error<T>(
    result: Result<T, rand::Error>,
) -> Result<T, arbtest::arbitrary::Error> {
    extract_arb_error(result).expect("RNG should only throw arbitrary::Error type")
}
