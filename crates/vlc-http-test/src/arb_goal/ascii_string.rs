// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
#[derive(Clone, Debug)]
pub struct AsciiString(pub mut(self) String);
impl<'a> arbitrary::Arbitrary<'a> for AsciiString {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let string: Result<_, _> = u
            .arbitrary_iter()?
            .map(|byte: Result<u8, _>| {
                const ASCII_NON_CONTROL_RANGE: std::ops::RangeInclusive<u8> = 0x20..=0x7E;
                let range_len = 1 + ASCII_NON_CONTROL_RANGE.end() - ASCII_NON_CONTROL_RANGE.start();
                let byte = byte?;
                let ascii_non_control_byte = (byte % range_len) + ASCII_NON_CONTROL_RANGE.start();
                // let ascii_byte = byte & 0x7F;
                Ok(ascii_non_control_byte as char)
            })
            .collect();
        let string: String = string?;
        assert!(string.is_ascii(), "non-ascii: {string:?}");
        Ok(Self(string))
    }
}
