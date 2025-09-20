// Copyright (C) 2021-2025  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! See [`ArgSplit`] for details.

use std::borrow::Cow;

pub(super) const BACKSLASH: char = '\\';
pub(super) const DOUBLE_QUOTE: char = '"';
pub(super) const SINGLE_QUOTE: char = '\'';

/// Parses a char sequence into arguments. Accepts quoted arguments and respects simple escaping.
///
/// Note: Due to escape sequences, this type must re-allocate an owned `String` for each token.
/// (e.g. the output tokens might not exist literally as a slice of the input)
///
/// # Example
/// ```
/// use arg_util::ArgSplit;
///
/// let split = ArgSplit::split(r#"these are some "quoted arguments""#);
/// assert_eq!(split, vec![
///     "these".to_string(),
///     "are".to_string(),
///     "some".to_string(),
///     "quoted arguments".to_string(),
/// ])
/// ```
#[derive(Debug, Default)]
pub struct ArgSplit<'a> {
    input: &'a str,
    /// Completed tokens
    tokens: Vec<Cow<'a, str>>,
    /// Pending token
    next_token: Option<NextToken>,
    /// Whether escape sequence is active
    escape_flag: Option<()>,
    /// Type of active quote
    quote_flag: Option<char>,
}
#[derive(Debug)]
enum NextToken {
    Owned(String),
    Borrowed(usize),
}
impl NextToken {
    fn convert_to_cow(self, input: &str, index: usize) -> Option<Cow<'_, str>> {
        match self {
            Self::Owned(owned) => Some(Cow::Owned(owned)),
            Self::Borrowed(start) => {
                if self.is_empty(index) {
                    None
                } else {
                    let token = &input[start..index];
                    Some(Cow::Borrowed(token))
                }
            }
        }
    }
    fn push_if_owned(&mut self, c: char) {
        match self {
            Self::Owned(next_token) => next_token.push(c),
            Self::Borrowed(_start) => {}
        }
    }
    fn is_empty(&self, index: usize) -> bool {
        match self {
            NextToken::Owned(owned) => owned.is_empty(),
            NextToken::Borrowed(start) => *start == index,
        }
    }
}
impl<'a> ArgSplit<'a> {
    /// Splits the specified string into owned arguments
    pub fn split_into_owned(input: &'a str) -> Vec<String> {
        Self::split(input)
            .into_iter()
            .map(std::borrow::Cow::into_owned)
            .collect()
    }
    /// Splits the specified string into arguments
    #[must_use]
    pub fn split(input: &'a str) -> Vec<Cow<'a, str>> {
        let mut state = Self {
            input,
            ..Self::default()
        };
        for char_index in input.char_indices() {
            state.push(char_index);
        }
        state.finish()
    }
    /// Process the next `char`
    fn push(&mut self, char_index: (usize, char)) {
        let (index, c) = char_index;
        let next_token = self.next_token.get_or_insert(NextToken::Borrowed(index));
        let need_owned = match c {
            _ if self.escape_flag.is_some() => {
                // accept any character escaped as itself (relaxed escape logic)
                self.escape_flag.take();
                next_token.push_if_owned(c);
                false
            }
            BACKSLASH if self.escape_flag.is_none() => {
                // START escape
                self.escape_flag = Some(());
                let is_token_empty = next_token.is_empty(index);
                if is_token_empty {
                    self.next_token.take();
                }
                !is_token_empty
            }
            DOUBLE_QUOTE | SINGLE_QUOTE if self.quote_flag.is_none() => {
                // OPEN quote
                self.quote_flag = Some(c);
                true
            }
            c if Some(c) == self.quote_flag => {
                // CLOSE quote
                self.quote_flag.take();
                true
            }
            c if c.is_ascii_whitespace() && self.quote_flag.is_none() => {
                self.end_token(index);
                false
            }
            _ => {
                next_token.push_if_owned(c);
                false
            }
        };
        if need_owned && let Some(NextToken::Borrowed(start)) = &self.next_token {
            let from_start = self.input[*start..index].to_string();
            self.next_token = Some(NextToken::Owned(from_start));
        }
    }
    /// Finalize `next_token`, and add the value to `tokens`
    fn end_token(&mut self, index: usize) {
        let ended_token: Option<Cow<'_, str>> = self
            .next_token
            .take()
            .and_then(|token| token.convert_to_cow(self.input, index));
        self.tokens.extend(ended_token);
    }
    /// Finish the split and return the final `tokens`
    fn finish(mut self) -> Vec<Cow<'a, str>> {
        self.end_token(self.input.len());
        self.tokens
    }
}

#[cfg(test)]
mod tests {
    use super::ArgSplit;

    macro_rules! test {
        (
            $(
                $input:tt $(=> $($output:expr),+)?
            );+ $(;)?
        ) => {
            $(
                test!(
                    @inner $input $(=> $($output),+)?
                );
            )+
        };
        (@inner $input:expr => $($output:expr),+) => {
            assert_eq!(ArgSplit::split($input), vec![$($output),+]);
        };
        (@inner $input:expr) => {
            assert_eq!(ArgSplit::split($input), Vec::<String>::new());
        };
    }

    #[test]
    fn unchanged() {
        test! {
            "this-is-unchanged" => "this-is-unchanged";
            "also" => "also";
            "th12423432is" => "th12423432is";
        }
    }

    #[test]
    fn regular_spaces() {
        test! {
            "a b c" => "a", "b", "c";
            "split this string into words" => "split", "this", "string", "into", "words";
            "  and    remove extra     whitespace   " => "and", "remove", "extra", "whitespace";
        }
    }

    #[test]
    fn escaped_spaces() {
        test! {
            r"escape\ space" => "escape space";
            r"some spaces\ arent\ skipped\ \ \ either\  yeah " => "some", "spaces arent skipped   either ", "yeah";
        }
    }

    #[test]
    fn escaped_backslash() {
        test! {
            r"escaped\\back\\slash" => r"escaped\back\slash";
            r"\\/\\/\\/\\/ /\\/\\/\\/\\" => r"\/\/\/\/", r"/\/\/\/\";
        }
    }

    #[test]
    fn escape_accepts_nonstandard() {
        test! {
            r"why\ not\ \e\s\c\a\p\e \e\v\e\r\y \l\e\t\t\e\r" => "why not escape", "every", "letter";
        }
    }

    #[test]
    fn quotes() {
        test! {
            r#""whole part""# => "whole part";
            r#"a b "c d e" f"# => "a", "b", "c d e", "f";
        }
    }

    #[test]
    fn quotes_interspersed() {
        test! {
            r#"first "double quote's" and then 'single "quote"s'"# => "first", "double quote's", "and", "then", r#"single "quote"s"#;
            r#" a " ' " ' b " ' " ' c "# => "a", " ' ", r#" b " "#, " ' c ";
            r#" a ' " ' " b ' " ' " c "# => "a", r#" " "#, " b ' ", r#" " c "#;
        }
    }
    #[test]
    fn leading_trailing_quote() {
        test! {
            "'a" => "a";
            "a'" => "a";
            "'" => "";
            r#""a"# => "a";
            r#"a""# => "a";
            r#"""# => "";
        }
    }

    #[test]
    fn single_quote_and_spaces() {
        test! {
            r"some spaces\ aren\'t\ skipped\ \ \ neither single_quotes\  yeah " => "some", "spaces aren't skipped   neither", "single_quotes ", "yeah";
        }
    }

    #[test]
    fn ignores_trailing_backslash() {
        test! {
            r"a\ b\" => "a b";
            r"sometimes you just trail off \" => "sometimes", "you", "just", "trail", "off";
        }
    }

    #[test]
    fn persists_empty_quotes() {
        test! {
            r#""""# => "";
            r#"empty looks like """# => "empty", "looks", "like", "";
            r#"empty looks like ""     "# => "empty", "looks", "like", "";
        }
    }

    #[test]
    fn unicode_escapes() {
        test! {
            "\u{99}" => "\u{99}";
            "a\u{99}" => "a\u{99}";
            "\u{99}b" => "\u{99}b";
        }
    }
}
