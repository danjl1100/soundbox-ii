// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
#[derive(Clone, Debug)]
pub struct AlphanumString(String);
impl<'a> arbitrary::Arbitrary<'a> for AlphanumString {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let string: Result<_, _> = u
            .arbitrary_iter()?
            .map(|byte: Result<u8, _>| {
                let ascii_byte = match byte? & 63 {
                    v @ 0..26 => b'a' + v,
                    v @ 26..52 => b'A' + (v - 26),
                    v @ 52..62 => b'0' + (v - 52),
                    62 => b'_',
                    63 => b'.',
                    _ => unreachable!(),
                };
                Ok(ascii_byte as char)
            })
            .collect();
        let string: String = string?;
        for c in string.chars() {
            match c {
                _ if c.is_alphanumeric() => {}
                '_' | '.' => {}
                _ => panic!("invalid char {c:?} in AlphanumString"),
            }
        }
        Ok(Self(string))
    }
}
impl std::ops::Deref for AlphanumString {
    type Target = String;
    fn deref(&self) -> &Self::Target {
        let Self(inner) = self;
        inner
    }
}
impl std::fmt::Display for AlphanumString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self(inner) = self;
        write!(f, "{inner}")
    }
}
