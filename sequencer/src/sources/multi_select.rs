// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Multiple-select adapter for various [`ItemSource`](`super::ItemSource`)s
use super::{beet, file};

/// Creates a multiple-select adapter for various heterogeneous [`ItemSource`](`super::ItemSource`)s
///
/// # Example
/// ```
/// #[macro_use]
/// use sequencer::source_multi_select;
/// use sequencer::sources::{Beet, FileLines, FolderListing};
///
/// source_multi_select! {
///     /// Multiple-select for various [`ItemSource`]s
///     pub struct MultiSelectCustom {
///         type Args = TypedArgsCustom<'a>;
///         /// The type of ItemSource
///         type Type = TypeCustom;
///         /// Beet
///         beet: Beet as Beet where arg type = Vec<String>,
///         /// FileLines
///         file_lines: FileLines as FileLines where arg type = String,
///         /// FolderListing
///         folder_listing: FolderListing as FolderListing where arg type = String,
///     }
///     /// Custom typed arg
///     impl ItemSource<Option<TypedArgCustom>> {
///         type Item = String;
///         /// This is the custom error type
///         type Error = ErrorCustom;
///     }
/// }
/// ```
#[allow(clippy::module_name_repetitions)]
#[macro_export]
macro_rules! source_multi_select {
    (
        $(#[$struct_meta:meta])*
        $vis:vis struct $struct:ident {
            $(#[$args_meta:meta])*
            type Args = $args:ident<'a>;
            $(#[$type_meta:meta])*
            type Type = $type:ident;
            $(
                $(#[$field_meta:meta])*
                $field_vis:vis $field:ident : $source_ty:ty
                    as $field_variant:ident
                    where arg type = $source_arg:ty
            ),+ $(,)?
        }
        $(#[$arg_meta:meta])*
        impl ItemSource<Option<$arg:ident>> {
            type Item = $item:ty;
            $(#[$error_meta:meta])*
            type Error = $error:ident;
        }
    ) => {
        $(#[$struct_meta])*
        $vis struct $struct {
            $(
                $(#[$field_meta])*
                $field_vis $field : $source_ty
            ),+
        }
        $(#[$arg_meta])*
        $vis enum $arg {
            $(
                $(#[$field_meta])*
                $field_variant ( $source_arg ),
            )+
        }
        $(#[$args_meta])*
        enum $args <'a> {
            $(
                $field_variant ( Vec<&'a $source_arg > ),
            )+
        }
        $(#[$error_meta])*
        $vis enum $error {
            $(
                $(#[$field_meta])*
                $field_variant(<$source_ty as $crate::sources::ItemSource<$source_arg>>::Error),
            )+
            /// Mismatch in argument types
            TypeMismatch($crate::sources::multi_select::Mismatch<$type>),
            /// No type for any arguments
            NoType,
        }
        $(#[$type_meta])*
        #[derive(Debug, PartialEq, Eq)]
        $vis enum $type {
            $(
                $(#[$field_meta])*
                $field_variant
            ),+
        }
        impl From<&$arg> for $type {
            fn from(arg: &$arg) -> Self {
                match arg {
                    $(
                        $arg::$field_variant(..) => $type::$field_variant,
                    )+
                }
            }
        }
        impl<'a> From<&$args<'a>> for $type {
            fn from(args: &$args<'a>) -> Self {
                match args {
                    $(
                        $args::$field_variant(..) => $type::$field_variant,
                    )+
                }
            }
        }
        impl<'a> $args<'a> {
            fn push(&mut self, arg: &'a $arg) -> Result<(), $crate::sources::multi_select::Mismatch<$type>> {
                match (self, arg) {
                    $(
                        (Self::$field_variant(args), $arg::$field_variant(arg)) => { args.push(arg); Ok(()) }
                    )+
                    (args, arg) => Err($crate::sources::multi_select::Mismatch {
                        found: arg.into(),
                        expected: (&*args).into(),
                    }),
                }
            }
        }
        impl<'a> From<&'a $arg> for $args<'a> {
            fn from(arg: &'a $arg) -> Self {
                match arg {
                    $(
                        $arg::$field_variant(arg) => Self::$field_variant(vec![arg]),
                    )+
                }
            }
        }
        impl<'a, 'b: 'a> TryFrom<&'b [Option<$arg>]> for $args<'a> {
            type Error = Option<$crate::sources::multi_select::Mismatch<$type>>;

            fn try_from(args: &'b [Option<$arg>]) -> Result<Self, Self::Error> {
                let mut typed_args = None;
                for arg_opt in args {
                    match (arg_opt, &mut typed_args) {
                        (None, _) => {
                            // continue
                        }
                        (Some(arg), None) => {
                            typed_args = Some($args::from(arg));
                        }
                        (Some(arg), Some(args)) => {
                            args.push(arg)?;
                        }
                    }
                }
                typed_args.ok_or(None)
            }
        }
        impl std::fmt::Display for $error {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    $(
                        Self::$field_variant(err) => write!(f, "{}", err),
                    )+
                    Self::TypeMismatch(mismatch) => write!(f, "arg type {}", mismatch),
                    Self::NoType => write!(f, "args untyped"),
                }
            }
        }
        impl $crate::sources::ItemSource<Option<$arg>> for $struct {
            type Item = $item;
            type Error = $error;

            fn lookup(&self, args: &[Option<$arg>]) -> Result<Vec<Self::Item>, Self::Error> {
                let typed_args =
                    $args::try_from(args).map_err(|e| e.map_or($error::NoType, $error::TypeMismatch))?;
                match typed_args {
                    $(
                        $args::$field_variant(args) => Ok(self.$field.lookup(&args).map_err($error::$field_variant)?),
                    )+
                }
            }
        }
    };
}

source_multi_select! {
    /// Multiple-select for various [`ItemSource`](`super::ItemSource`)s
    pub struct MultiSelectCustom {
        type Args = TypedArgsCustom<'a>;
        /// The type of ItemSource
        type Type = TypeCustom;
        /// Beet
        beet: beet::Beet as Beet where arg type = Vec<String>,
        /// FileLines
        file_lines: file::Lines as FileLines where arg type = String,
        /// FolderListing
        folder_listing: file::FolderListing as FolderListing where arg type = String,
    }
    /// Custom typed arg
    impl ItemSource<Option<TypedArgCustom>> {
        type Item = String;
        /// This is the custom error type
        type Error = ErrorCustom;
    }
}

// /// Selects between multiple [`ItemSource`]s
// ///
// /// - [`beet::Beet`]
// /// - [`file::Lines`]
// pub struct MultiSelect {
//     file_lines: file::Lines,
//     beet: beet::Beet,
// }
// type BeetArg = Vec<String>;
// type FileLinesArg = String;
// type Arg = Option<TypedArg>;
/// Mismatch in types
#[allow(missing_docs)]
pub struct Mismatch<T> {
    pub found: T,
    pub expected: T,
}
impl<T> Mismatch<T>
where
    T: Eq,
{
    /// Combines the specified optional values, verifying values match if both are specified
    ///
    /// # Errors
    /// Returns a `Mismatch` error if both values are present and not equal
    pub fn combine_verify(found: Option<T>, expected: Option<T>) -> Result<Option<T>, Self> {
        match (found, expected) {
            (Some(lhs), Some(rhs)) if lhs == rhs => Ok(Some(lhs)),
            (Some(lhs), None) => Ok(Some(lhs)),
            (None, Some(rhs)) => Ok(Some(rhs)),
            (None, None) => Ok(None),
            (Some(found), Some(expected)) => Err(Mismatch { found, expected }),
        }
    }
}
impl<T> std::fmt::Display for Mismatch<T>
where
    T: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Mismatch { found, expected } = self;
        write!(f, "mismatch: found {found:?}, expected {expected:?}")
    }
}
// shared::wrapper_enum! {
//     enum TypedArg {
//         Beet(BeetArg),
//         FileLines(FileLinesArg),
//     }
//     enum Error {
//         Beet(<beet::Beet as ItemSource<BeetArg>>::Error),
//         FileLines(<file::Lines as ItemSource<FileLinesArg>>::Error),
//         TypeMismatch(Mismatch<Type>),
//         { impl None for }
//         NoType,
//     }
// }
// enum TypedArgs<'a> {
//     Beet(Vec<&'a BeetArg>),
//     FileLines(Vec<&'a FileLinesArg>),
// }
// #[derive(Copy, Clone, Debug, PartialEq)]
// enum Type {
//     Beet,
//     FileLines,
// }
// impl From<&TypedArg> for Type {
//     fn from(arg: &TypedArg) -> Self {
//         match arg {
//             TypedArg::Beet(..) => Type::Beet,
//             TypedArg::FileLines(..) => Type::FileLines,
//         }
//     }
// }
// impl<'a> From<&TypedArgs<'a>> for Type {
//     fn from(arg: &TypedArgs<'a>) -> Self {
//         match arg {
//             TypedArgs::Beet(..) => Type::Beet,
//             TypedArgs::FileLines(..) => Type::FileLines,
//         }
//     }
// }
// impl<'a> From<Type> for TypedArgs<'a> {
//     fn from(ty: Type) -> Self {
//         match ty {
//             Type::Beet => Self::Beet(vec![]),
//             Type::FileLines => Self::FileLines(vec![]),
//         }
//     }
// }
// impl<'a> TryFrom<&'a [Arg]> for TypedArgs<'a> {
//     type Error = Option<Mismatch<Type>>;
//
//     fn try_from(args: &[Arg]) -> Result<Self, Self::Error> {
//         let mut typed_args = None;
//         for arg in args {
//             let ty = arg.as_ref().map(Type::from);
//             let common_ty = typed_args.as_ref().map(Type::from);
//             match (ty, common_ty) {
//                 (Some(ty), None) => {
//                     // fill common (first typed arg)
//                     typed_args = Some(TypedArgs::from(ty));
//                 }
//                 (Some(ty), Some(common)) if ty != common => {
//                     // conflict - type mismatch
//                     return Err(Some(Mismatch {
//                         found: ty,
//                         expected: common,
//                     }));
//                 }
//                 (None, _) | (Some(_), Some(_)) => {
//                     // compatible types
//                 }
//             }
//         }
//         typed_args.ok_or(None)
//     }
// }
// impl std::fmt::Display for Error {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         match self {
//             Self::Beet(err) => write!(f, "{err}"),
//             Self::FileLines(err) => write!(f, "{err}"),
//             Self::TypeMismatch(Mismatch { found, expected }) => {
//                 write!(
//                     f,
//                     "arg type mismatch: found {found:?}, expected {expected:?}"
//                 )
//             }
//             Self::NoType => write!(f, "args untyped"),
//         }
//     }
// }
// impl ItemSource<Arg> for MultiSelect {
//     type Item = String;
//     type Error = Error;
//
//     fn lookup(&self, args: &[Arg]) -> Result<Vec<Self::Item>, Self::Error> {
//         let typed_args =
//             TypedArgs::try_from(args).map_err(|e| e.map_or(Error::NoType, Error::from))?;
//         match typed_args {
//             TypedArgs::Beet(args) => Ok(self.beet.lookup(&args)?),
//             TypedArgs::FileLines(args) => Ok(self.file_lines.lookup(&args)?),
//         }
//     }
// }
