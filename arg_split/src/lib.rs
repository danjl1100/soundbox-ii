// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! See [`ArgSplit`] for details.

use std::iter::FromIterator;

const BACKSLASH: char = '\\';
const DOUBLE_QUOTE: char = '"';
const SINGLE_QUOTE: char = '\'';

/// Parses a char sequence into arguments. Accepts quoted arguments and respects simple escaping.
///
/// # Example
/// ```
/// use arg_split::ArgSplit;
///
/// let split = ArgSplit::split(r#"these are some "quoted arguments""#);
/// assert_eq!(split, vec![
///     "these".to_string(),
///     "are".to_string(),
///     "some".to_string(),
///     "quoted arguments".to_string(),
/// ])
///
/// ```
#[derive(Default)]
pub struct ArgSplit {
    /// Completed tokens
    tokens: Vec<String>,
    /// Pending token
    next_token: String,
    /// Whether escape sequence is active
    escape_flag: Option<()>,
    /// Type of active quote
    quote_flag: Option<char>,
}
impl ArgSplit {
    /// Splits the specified string into arguments
    pub fn split(input: &str) -> Vec<String> {
        input.chars().collect::<Self>().finish()
    }
    /// Process the next `char`
    fn push(&mut self, c: char) {
        match c {
            _ if self.escape_flag.is_some() => {
                // accept any character escaped as itself (relaxed escape logic)
                self.escape_flag.take();
                self.next_token.push(c);
            }
            BACKSLASH if self.escape_flag.is_none() => {
                // START escape
                self.escape_flag = Some(());
            }
            DOUBLE_QUOTE | SINGLE_QUOTE if self.quote_flag.is_none() => {
                // OPEN quote
                self.quote_flag = Some(c);
            }
            c if Some(c) == self.quote_flag => {
                // CLOSE quote
                self.quote_flag.take();
            }
            c if c.is_ascii_whitespace() && self.quote_flag.is_none() => {
                self.end_token();
            }
            _ => {
                self.next_token.push(c);
            }
        }
    }
    /// Finalize `next_token`, and add the value to `tokens`
    fn end_token(&mut self) {
        if !self.next_token.is_empty() {
            let token = std::mem::take(&mut self.next_token);
            self.tokens.push(token);
        }
    }
    /// Finish the split and return the final `tokens`
    fn finish(mut self) -> Vec<String> {
        self.end_token();
        self.tokens
    }
}
impl FromIterator<char> for ArgSplit {
    fn from_iter<T: IntoIterator<Item = char>>(iter: T) -> Self {
        let mut state = Self::default();
        for c in iter {
            state.push(c);
        }
        state
    }
}

#[cfg(test)]
mod tests {
    use super::ArgSplit;

    #[test]
    fn doctest() {}

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
            "'";
            r#""a"# => "a";
            r#"a""# => "a";
            r#"""#;
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
}
