// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
/// Adapter for [`rand::RngCore`]
///
/// This allows for reporting custom errors (e.g. [`arbitrary::Error`]) for tests.
///
/// Users generally provide [`rand::RngCore`] by wrapping in the [`ErrorRng`] struct
pub trait ArbitrarySource {
    /// Error generating arbitrary values
    type Error: std::error::Error;
    /// Fills the specified slice with arbitrary bytes
    ///
    /// # Errors
    /// Returns an error if the source extraction fails
    fn try_fill(&mut self, dest: &mut [u8]) -> Result<(), Self::Error>;
}

/// Random Number Generator (RNG) wrapper for an error-compatible version of [`rand`]
pub struct ErrorRng<'a, R: ?Sized>(pub &'a mut R);
impl<R: ?Sized> ArbitrarySource for ErrorRng<'_, R>
where
    R: rand::RngCore,
{
    type Error = rand::Error;

    fn try_fill(&mut self, dest: &mut [u8]) -> Result<(), Self::Error> {
        use rand::Rng as _;

        let Self(rng) = self;
        rng.try_fill(dest)
    }
}

// TODO: pending removal of non-optional WASI dependencies,
// see <https://github.com/rust-random/getrandom/pull/830>
// /// Random Number Generator (RNG) wrapper for a panicking version of [`rand_next`]
// pub struct PanicRng<'a, R: ?Sized>(pub &'a mut R);
// impl<R: ?Sized> ArbitrarySource for PanicRng<'_, R>
// where
//     R: rand_next::Rng,
// {
//     type Error = std::convert::Infallible;
//
//     fn try_fill(&mut self, dest: &mut [u8]) -> Result<(), Self::Error> {
//         let Self(rng) = self;
//         rand_next::Fill::fill_slice(dest, rng);
//         Ok(())
//     }
// }
