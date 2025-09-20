// Copyright (C) 2021-2025  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Counterpart to [`crate::ArgSplit`]

use super::arg_split::{BACKSLASH, DOUBLE_QUOTE, SINGLE_QUOTE};

/// Creates a string joining the [`std::fmt::Display`] items separated by spaces
///
/// # Examples
///
/// ```
/// use arg_util::{join_display_by_spaces};
/// let items = vec!["a", "b", "c"];
/// assert_eq!(join_display_by_spaces(items), "a b c");
/// ```
pub fn join_display_by_spaces<T>(items: impl IntoIterator<Item = T>) -> String
where
    T: std::fmt::Display,
{
    items.into_iter().fold(String::new(), |mut buf, line| {
        use std::fmt::Write as _;
        if !buf.is_empty() {
            write!(buf, " ").expect("string format is infallible");
        }
        write!(buf, "{line}").expect("string format is infallible");
        buf
    })
}

/// Creates a string joining the [`std::fmt::Debug`] items separated by spaces
///
/// # Examples
///
/// ```
/// use arg_util::{join_debug_by_spaces};
/// let items = vec!["a", "b", "c"];
/// assert_eq!(join_debug_by_spaces(items), r#""a" "b" "c""#);
/// ```
///
/// ```
/// use arg_util::{join_debug_by_spaces, StrDebugSplit};
/// let items = ["a", "b", "c d e f"].into_iter().map(StrDebugSplit::new);
/// assert_eq!(join_debug_by_spaces(items), r#"a b "c d e f""#);
/// ```
pub fn join_debug_by_spaces<T>(items: impl IntoIterator<Item = T>) -> String
where
    T: std::fmt::Debug,
{
    items.into_iter().fold(String::new(), |mut buf, line| {
        use std::fmt::Write as _;
        if !buf.is_empty() {
            write!(buf, " ").expect("string format is infallible");
        }
        write!(buf, "{line:?}").expect("string format is infallible");
        buf
    })
}

/// Wrapper for [`String`] where the [`std::fmt::Debug`] implementation is compatible with
/// [`crate::ArgSplit`] parsing
#[derive(Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct StrDebugSplit<'a> {
    inner: &'a str,
    #[serde(skip)]
    has_spaces: bool,
}
impl std::fmt::Debug for StrDebugSplit<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { inner, has_spaces } = *self;
        let quoted = has_spaces || inner.is_empty();
        if quoted {
            write!(f, "\"")?;
        }
        for c in inner.chars() {
            match c {
                BACKSLASH | SINGLE_QUOTE | DOUBLE_QUOTE => write!(f, r"\{c}"),
                _ => write!(f, "{c}"),
            }?;
            // TODO
        }
        if quoted {
            write!(f, "\"")?;
        }
        Ok(())
    }
}

// Helper conversions
impl<'a> StrDebugSplit<'a> {
    /// Constructs the wrapper, with a linear check for whitespace
    pub fn new(inner: &'a str) -> Self {
        let has_spaces = inner.chars().any(char::is_whitespace);
        Self { inner, has_spaces }
    }
}
impl<'a> From<StrDebugSplit<'a>> for &'a str {
    fn from(value: StrDebugSplit<'a>) -> Self {
        let StrDebugSplit { inner, .. } = value;
        inner
    }
}
impl AsRef<str> for StrDebugSplit<'_> {
    fn as_ref(&self) -> &str {
        (*self).into()
    }
}

/// Owned version of [`StrDebugSplit`]
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, arbitrary::Arbitrary)]
#[serde(transparent)]
pub struct StringDebugSplit(String);
impl From<String> for StringDebugSplit {
    fn from(value: String) -> Self {
        Self(value)
    }
}
impl From<StringDebugSplit> for String {
    fn from(value: StringDebugSplit) -> Self {
        let StringDebugSplit(inner) = value;
        inner
    }
}
impl std::fmt::Debug for StringDebugSplit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self(inner) = self;
        let inner = StrDebugSplit::new(inner);
        <StrDebugSplit as std::fmt::Debug>::fmt(&inner, f)
    }
}
impl std::str::FromStr for StringDebugSplit {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(s.to_owned().into())
    }
}
impl AsRef<str> for StringDebugSplit {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::join_debug_by_spaces;
    use crate::{ArgSplit, StrDebugSplit, StringDebugSplit};

    fn debug_carefully<T>(values: &[T])
    where
        T: std::fmt::Debug + AsRef<str>,
    {
        if values.is_empty() {
            dbg!(values.is_empty());
        }
        for (index, elem) in values.iter().enumerate() {
            dbg!((index, elem.as_ref()));
        }
    }

    fn check<T>(values: &[T])
    where
        T: std::fmt::Debug + AsRef<str>,
    {
        println!("{:-<80}", "");

        debug_carefully(values);
        let combined_str = join_debug_by_spaces(values);

        let separated = ArgSplit::split_into_owned(&combined_str);
        debug_carefully(&separated);

        assert_eq!(values.len(), separated.len());
        for (orig, reparsed) in values.iter().zip(separated) {
            assert_eq!(orig.as_ref(), reparsed);
        }
    }

    fn check_strs(strs: &[&str]) {
        check(
            &strs
                .iter()
                .map(|s| StrDebugSplit::new(s))
                .collect::<Vec<_>>(),
        );
    }

    #[test]
    fn literal_slashes() {
        check_strs(&[r"\C"]);
    }
    #[test]
    fn literal_quote_single() {
        check_strs(&["'"]);
    }
    #[test]
    fn literal_quote_double() {
        check_strs(&[r#"""#]);
    }
    #[test]
    fn whitespace_needs_quotes() {
        assert_eq!(format!("{:?}", StrDebugSplit::new("-\nXI")), "\"-\nXI\"");
    }

    #[test]
    fn arb_debug_split() {
        arbtest::arbtest(|u| {
            let strs: Vec<String> = u.arbitrary()?;

            let uut_borrowed: Vec<_> = strs.iter().map(|s| StrDebugSplit::new(s)).collect();
            check(&uut_borrowed);

            let uut_owned: Vec<_> = strs.into_iter().map(StringDebugSplit::from).collect();
            check(&uut_owned);

            Ok(())
        });
    }
}
