// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use crate::order::ArbitrarySource;
use std::collections::VecDeque;

/// Random Number Generator that is fed by a deterministic `arbtest::arbitrary`
pub(crate) struct ArbitraryRng<'a, 'b>(&'a mut arbtest::arbitrary::Unstructured<'b>)
where
    'b: 'a;

impl ArbitrarySource for ArbitraryRng<'_, '_> {
    type Error = arbtest::arbitrary::Error;

    fn try_fill(&mut self, dest: &mut [u8]) -> Result<(), Self::Error> {
        for dest in dest {
            *dest = self.0.arbitrary()?;
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
impl ArbitrarySource for PanicRng {
    type Error = std::convert::Infallible;
    fn try_fill(&mut self, _dest: &mut [u8]) -> Result<(), Self::Error> {
        unreachable!("try_fill in PanicRng");
    }
}
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

pub(crate) fn decode_hex(strs: &[impl AsRef<str>]) -> Result<Vec<u8>, std::num::ParseIntError> {
    strs.iter()
        .flat_map(|s| {
            let s = s.as_ref();
            assert!(s.len() % 2 == 0, "hex str should have even length");
            (0..s.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&s[i..i + 2], 16))
        })
        .collect()
}

#[derive(Default)]
pub(super) struct RngHolder(Option<VecDeque<u8>>);

impl RngHolder {
    /// Returns an Error if the bytes are already set, or the input is not valid hex
    pub fn set_bytes(
        &mut self,
        bytes_hex: &[impl AsRef<str>],
    ) -> Result<(), Option<std::num::ParseIntError>> {
        if self.0.is_some() {
            return Err(None);
        }

        let mut bytes = VecDeque::from(decode_hex(bytes_hex)?);
        bytes.make_contiguous();
        self.0 = Some(bytes);

        Ok(())
    }
    pub fn get_bytes(&self) -> &[u8] {
        if let Some(bytes) = &self.0 {
            let (bytes_contiguous, empty) = bytes.as_slices();
            assert!(
                empty.is_empty(),
                "second slice should be empty after VecDeque::make_contiguous"
            );
            bytes_contiguous
        } else {
            &[]
        }
    }
    pub fn truncate_from_left(&mut self, len_new: usize) {
        let Some(bytes) = self.0.as_mut() else {
            panic!("truncate should act on Enabled RngHolder");
        };

        let len_orig = bytes.len();
        assert!(
            len_new <= len_orig,
            "truncate_from_left should not increase length (len_orig {len_orig} -> len_new {len_new})"
        );

        bytes.rotate_left(len_orig - len_new);
        bytes.truncate(len_new);
    }
}
