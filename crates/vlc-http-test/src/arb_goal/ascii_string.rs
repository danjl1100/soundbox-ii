// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
#[derive(Clone, Debug)]
pub struct AsciiString(String);
impl<'a> arbitrary::Arbitrary<'a> for AsciiString {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let string: Result<_, _> = u
            .arbitrary_iter()?
            .map(|byte: Result<u8, _>| {
                let ascii_byte = byte? & 0x7F;
                Ok(ascii_byte as char)
            })
            .collect();
        let string: String = string?;
        assert!(string.is_ascii(), "non-ascii: {string:?}");
        Ok(Self(string))
    }
}
impl std::ops::Deref for AsciiString {
    type Target = String;
    fn deref(&self) -> &Self::Target {
        let Self(inner) = self;
        inner
    }
}
impl std::fmt::Display for AsciiString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self(inner) = self;
        write!(f, "{inner}")
    }
}
