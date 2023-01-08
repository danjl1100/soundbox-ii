// Copyright (C) 2021-2023  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Combines argument [`Value`]s coming from various [`Source`]s

/// Original source of a value
// NOTE: Itentionally not deriving Display, Clone, Copy
#[derive(Debug, PartialEq, Eq)]
pub enum Source {
    /// Command-line argument
    Cli,
    //TODO refactor Input, InputAndEnv to follow the hierarchy Cli > Env > Config > Default
    /// Environment variable
    Env,
    /// Config file
    Config,
    /// Default value
    Default,
}
/// Value with an associated source type
#[must_use]
pub struct Value<T>(pub T, pub Source);
const DEFAULT_BOOL: Value<bool> = Value::define_default(false);
impl<T> Value<T> {
    /// Transforms the inner value, while maintaining the source
    pub fn map<F, U>(self, map_fn: F) -> Value<U>
    where
        F: FnOnce(T) -> U,
    {
        let Self(value, source) = self;
        Value(map_fn(value), source)
    }
    /// Transforms the inner value, maintaining the source if the mapped value is `Result::Ok`
    ///
    /// # Errors
    /// Returns an error if the `map_fn` errors
    pub fn and_then<F, U, E>(self, map_fn: F) -> Result<Value<U>, E>
    where
        F: FnOnce(T) -> Result<U, E>,
    {
        let Self(value, source) = self;
        map_fn(value).map(|value| Value(value, source))
    }
    /// Defines a default-typed value
    pub const fn define_default(value: T) -> Self {
        Self(value, Source::Default)
    }
    /// Convenience function for referencing the inner value
    pub fn inner(&self) -> &T {
        &self.0
    }
    /// Convenience function for moving the inner value
    pub fn into_inner(self) -> T {
        self.0
    }
    /// Converts a nested option to a flat option
    pub fn flatten(outer: Option<Value<Option<T>>>) -> Option<Value<T>> {
        outer.and_then(|value| {
            let Value(inner, source) = value;
            inner.map(|value| Value(value, source))
        })
    }
}
// TODO remove if unused
// impl<T, E> Value<Result<T, E>> {
//     /// Converts an inner result to an outer result (this removing the `source` if an error
//     /// occurred
//     fn transpose(self) -> Result<Value<T>, E> {
//         let Self(result, source) = self;
//         result.map(|value| Value(value, source))
//     }
// }
impl<T> std::fmt::Display for Value<T>
where
    T: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self(inner, source) = self;
        write!(f, "{inner:?} (from {source})")
    }
}
impl std::fmt::Display for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Source::Cli => write!(f, "command-line argument"),
            Source::Env => write!(f, "environment variable"),
            Source::Config => write!(f, "config file"),
            Source::Default => write!(f, "default"),
        }
    }
}

/// Combination of data from cli and file
///
/// Note that environment variable data is not stored, as that data should only be generated if
/// the others are empty
///
/// Recommended use has `T` as a user-defined struct, instantiated by user input fed to `clap` or `serde` crates.
///
/// See the [`derive_unpack`](`crate::derive_unpack`) macro for usage
#[derive(Debug, PartialEq, Eq)]
pub struct Input<T> {
    /// Command-line provided data
    pub cli_args: T,
    /// Config file provided data
    pub file_args: T,
}
impl<T> Input<T> {
    /// Returns the first value for which the `accept_fn` returns `true`
    /// Note: Command-line argument has precedence over config file
    pub fn get_first_filter<F>(self, accept_fn: F) -> Option<Value<T>>
    where
        F: Fn(&T) -> bool,
    {
        if accept_fn(&self.cli_args) {
            Some(Value(self.cli_args, Source::Cli))
        } else if accept_fn(&self.file_args) {
            Some(Value(self.file_args, Source::Config))
        } else {
            None
        }
    }
    /// Constructs a combinator for the given environment variable name/key
    pub fn env(self, env_key: &'static str) -> InputAndEnv<T> {
        InputAndEnv {
            input: self,
            env_key,
        }
    }
}
impl<T> Input<Option<T>> {
    /// Returns the first value.
    /// Note: Precedence is given to command-line argument > config file
    ///
    /// To include a fallback environment variable, see [`Input::env`]
    pub fn get_first(self) -> Option<Value<T>> {
        Value::flatten(self.get_first_filter(Option::is_some))
    }
}
impl Input<bool> {
    /// Returns a logic OR of the available values
    pub fn or(self) -> Value<bool> {
        let Self {
            cli_args,
            file_args,
        } = self;
        if cli_args {
            Value(cli_args, Source::Cli)
        } else if file_args {
            Value(file_args, Source::Config)
        } else {
            DEFAULT_BOOL
        }
    }
}
// TODO this seems to violate the spirit of the Input struct contract
// impl<T> From<T> for Input<T>
// where
//     T: Copy,
// {
//     fn from(value: T) -> Self {
//         Self {
//             cli_args: value,
//             file_args: value,
//         }
//     }
// }
/// An [`Input`] with an associated fallback environment variable key
pub struct InputAndEnv<T> {
    input: Input<T>,
    env_key: &'static str,
}
impl<T> InputAndEnv<T> {
    fn get_env(key: &'static str) -> Option<Value<String>> {
        let value = std::env::var(key).ok();
        value.map(|inner| Value(inner, Source::Env))
    }
}
impl<T> InputAndEnv<Option<T>> {
    /// Returns the first value.
    /// Precedence is given to command-line argument > config file > environment variable
    ///
    /// See also: [`Self::get_first_str`], [`Self::or_parse_bool`]
    ///
    /// # Errors
    ///
    /// Returns a result if the environment variable is read, but fails parsing per the provided
    /// `env_parse_fn`
    pub fn try_get_first<F, E>(self, env_parse_fn: F) -> Result<Option<Value<T>>, E>
    where
        F: Fn(&'static str, String) -> Result<T, E>,
    {
        let env_parse_fn = |value| env_parse_fn(self.env_key, value);
        self.input
            .get_first()
            .map(Ok)
            .or_else(|| Self::get_env(self.env_key).map(|value| value.and_then(env_parse_fn)))
            .transpose()
    }
}
impl InputAndEnv<Option<String>> {
    /// Returns the first string value
    ///
    /// Convenience function for [`Self::try_get_first`], with a no-op parse function
    #[must_use]
    pub fn get_first_str(self) -> Option<Value<String>> {
        self.input
            .get_first()
            .or_else(|| Self::get_env(self.env_key))
    }
}
impl InputAndEnv<bool> {
    /// Returns the first value
    ///
    /// If no existing value found, parses the environment variable as a boolean
    /// (with the empty string and "0" as falsy values)
    pub fn or_parse_bool(self) -> Value<bool> {
        let parse_bool = |env_str: String| !matches!(env_str.as_str(), "" | "0");
        let input_result = self.input.or();
        if input_result.0 {
            input_result
        } else {
            Self::get_env(self.env_key).map_or(DEFAULT_BOOL, |value| value.map(parse_bool))
        }
    }
}

/// Derives an unpacked form of a struct
///
/// ```
/// use arg_util::{Input, Source, Value};
///
/// arg_util::derive_unpack! {
///     struct MyConfig impl unpacked as MyConfigUnpacked {
///         foo: Option<String>,
///         bar: bool,
///     }
/// }
///
/// let cli_args = MyConfig {
///     foo: Some("from cli".to_string()),
///     bar: false,
/// };
/// let file_args = MyConfig {
///     foo: None,
///     bar: true,
/// };
/// let input = Input { cli_args, file_args };
///
/// /// Unpack to handle each field separately
/// let MyConfigUnpacked { foo, bar } = input.into();
/// match foo.get_first() {
///     Some(Value(foo, source)) => {
///         assert_eq!(foo, "from cli");
///         assert_eq!(source, Source::Cli);
///     }
///     _ => panic!(),
/// }
/// let Value(bar, source) = bar.or();
/// assert_eq!(bar, true);
/// assert_eq!(source, Source::Config);
/// ```
#[macro_export]
macro_rules! derive_unpack {
    ($(
        $(#[$struct_meta:meta])*
        $struct_vis:vis struct $Struct:ident impl unpacked as $StructUnpacked:ident {
            $(
                $(#[$field_meta:meta])*
                $field_vis:vis $field:ident : $field_ty:ty
            ),+ $(,)?
        }
    )+) => {$(
        $(#[$struct_meta])*
        $struct_vis struct $Struct {
            $(
                $(#[$field_meta])*
                $field_vis $field : $field_ty
            ),+
        }
        $struct_vis struct $StructUnpacked {
            $(
                $field_vis $field : $crate::Input< $field_ty >
            ),+
        }
        impl From<$crate::Input<$Struct>> for $StructUnpacked {
            fn from(input: $crate::Input<$Struct>) -> Self {
                use $crate::Input;
                let Input {
                    cli_args,
                    file_args,
                } = input;
                Self {
                    $(
                        $field: Input {
                            cli_args: cli_args.$field,
                            file_args: file_args.$field,
                        },
                    )+
                }
            }
        }
    )+};
}
