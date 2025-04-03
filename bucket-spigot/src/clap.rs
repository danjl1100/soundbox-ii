// Copyright (C) 2021-2025  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! [`clap`] compatible versions of types

use crate::{modify_cmd_ref::ModifyCmdRef, path::Path};

// re-export `clap`
#[allow(clippy::module_name_repetitions, unused)]
pub use ::clap as clap_crate;
use std::str::FromStr;

/// Generic bounds required for all [`ModifyCmd`] type parameters
pub trait ArgBounds:
    FromStr<Err: std::error::Error + Send + Sync + 'static>
    + Clone
    + std::fmt::Debug
    + Send
    + Sync
    + 'static
{
}
impl<T> ArgBounds for T
where
    Self: FromStr + Clone + std::fmt::Debug + Send + Sync + 'static,
    Self::Err: std::error::Error + Send + Sync + 'static,
{
}

/// Command to modify a network, from the command-line
#[derive(Clone, clap::Subcommand, Debug, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum ModifyCmd<T, U>
where
    T: ArgBounds,
    U: ArgBounds,
{
    /// Add a new bucket
    AddBucket {
        /// Parent path for the new bucket
        parent: Path,
    },
    /// Add a new joint
    AddJoint {
        /// Parent path for the new joint
        parent: Path,
    },
    /// Delete a node (bucket/joint) that is empty
    DeleteEmpty {
        /// Path of the node (bucket/joint) to delete
        path: Path,
    },
    /// Set the contents of the specified bucket
    ///
    /// Removes the bucket from the "needing fill" list (if present)
    FillBucket {
        /// Path of the bucket to fill
        bucket: Path,
        /// Items for the bucket
        new_contents: Vec<T>,
    },
    /// Set the filters on a joint or bucket
    SetFilters {
        /// Path for the existing joint or bucket
        path: Path,
        /// List of filters to set
        new_filters: Vec<U>,
    },
    /// Set the weight on a joint or bucket
    SetWeight {
        /// Path for the existing joint or bucket
        path: Path,
        /// Weight value (relative to other weights on sibling nodes)
        new_weight: u32,
    },
    /// Set the ordering type for the joint or bucket
    SetOrderType {
        /// Path for the existing joint or bucket
        path: Path,
        /// Order type (how to select from immediate child nodes or items)
        new_order_type: OrderType,
    },
}
/// Ordering scheme for child nodes of a joint, or child items of a bucket
///
/// NOTE: Separate from [`crate::order::OrderType`] to emphasize `clap` as a public (string) interface
#[derive(Clone, Copy, clap::ValueEnum, Debug, serde::Serialize, serde::Deserialize)]
pub enum OrderType {
    /// Selects each child in turn, repeating each according to the weights
    InOrder,
    /// Selects a random (weighted) child
    Random,
    /// Selects from a randomized order of the children
    /// NOTE: For N total child-weight choices, the result is the shuffled version of
    /// [`InOrder`](`Self::InOrder`)
    Shuffle,
}

macro_rules! mirror_impl {

    // Simple structs (no generics)
    (impl From $order_ty_a:ty, $order_ty_b:ty {
        $($variant:ident),* $(,)?
    }) => {
        mirror_impl!(impl @one $order_ty_a, $order_ty_b { $($variant),* });
        mirror_impl!(impl @one $order_ty_b, $order_ty_a { $($variant),* });
    };
    (impl @one $ty_from:ty, $ty_to:ty {$($tt:tt)*}) => {
        impl From<$ty_from> for $ty_to {
            mirror_impl!{
                @fn from $ty_from = $ty_from, $ty_to {
                    $($tt)*
                }
            }
        }
    };

    // ModifyCmd specific generics
    (impl From ModifyCmd $modify_path_a:path = $modify_ty_a:ty, $modify_path_b:path = $modify_ty_b:ty {
        $($variant:ident {
            $($field:ident),* $(,)?
        }),* $(,)?
    }) => {
        mirror_impl!(impl @one $modify_path_a = $modify_ty_a, $modify_ty_b { $($variant { $($field),* },)* });
        mirror_impl!(impl @one $modify_path_b = $modify_ty_b, $modify_ty_a { $($variant { $($field),* },)* });
    };
    (impl @one $path_from:path = $ty_from:ty, $ty_to:ty { $($tt:tt)* }) => {
        impl<T, U> From<$ty_from> for $ty_to
        where
            T: ArgBounds,
            U: ArgBounds,
        {
            mirror_impl!( @fn from $path_from = $ty_from, $ty_to { $($tt)* } );
        }
    };

    // common `fn from`, uses `Into::into` for every field
    (@fn from $path_from:path = $ty_from:ty, $ty_to:ty {
        $($variant:ident
            $({
                $($field:ident),* $(,)?
            })?
        ),* $(,)?
    }) => {
        fn from(value: $ty_from) -> Self {
            use $path_from as Other;
            match value {
                $(
                    Other::$variant $({
                        $($field),*
                    })? => Self::$variant $({
                        $($field : $field.into()),*
                    })?
                ),*
            }
        }
    }

}

mirror_impl! {
    impl From crate::order::OrderType, self::OrderType {
        InOrder,
        Shuffle,
        Random,
    }
}
mirror_impl! {
    impl From ModifyCmd crate::ModifyCmd = crate::ModifyCmd<T, U>, self::ModifyCmd = self::ModifyCmd<T, U> {
        AddBucket { parent },
        AddJoint { parent },
        DeleteEmpty { path },
        FillBucket { bucket, new_contents },
        SetFilters { path, new_filters },
        SetWeight { path, new_weight },
        SetOrderType { path, new_order_type },
    }
}

impl<'a, T, U> ModifyCmdRef<'a, T, U>
where
    T: ArgBounds,
    U: ArgBounds,
{
    #[allow(unused)] // TODO for fn: as_command_lines
    pub(crate) fn display_as_cmd(self) -> impl std::fmt::Display + 'a {
        use crate::ModifyCmdRef as Other;
        use clap_crate::ValueEnum as _;

        struct Ret<'a, T, U>(Other<'a, T, U>)
        where
            T: ArgBounds,
            U: ArgBounds;
        impl<T, U> std::fmt::Display for Ret<'_, T, U>
        where
            T: ArgBounds,
            U: ArgBounds,
        {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self.0 {
                    Other::AddBucket { parent } => write!(f, "add-bucket {parent}"),
                    Other::AddJoint { parent } => write!(f, "add-joint {parent}"),
                    Other::DeleteEmpty { path } => write!(f, "delete-empty {path}"),
                    Other::FillBucket {
                        bucket,
                        new_contents,
                    } => {
                        write!(f, "fill-bucket {bucket}")?;
                        for item in new_contents {
                            write!(f, " {item:?}")?;
                        }
                        Ok(())
                    }
                    Other::SetFilters { path, new_filters } => {
                        write!(f, "set-filters {path}")?;
                        for filter in new_filters {
                            write!(f, " {filter:?}")?;
                        }
                        Ok(())
                    }
                    Other::SetWeight { path, new_weight } => {
                        write!(f, "set-weight {path} {new_weight}")
                    }
                    Other::SetOrderType {
                        path,
                        new_order_type,
                    } => {
                        let new_order_type = OrderType::from(new_order_type)
                            .to_possible_value()
                            .expect("no clap-skipped OrderTypes");
                        let new_order_type = new_order_type.get_name();

                        write!(f, "set-order-type {path} {new_order_type}")
                    }
                }
            }
        }
        Ret(self)
    }
}

pub use network_cmd_lines::Error as NetworkScriptError;
mod network_cmd_lines {
    use super::ArgBounds;
    use crate::Network;

    impl<T, U> Network<T, U>
    where
        T: ArgBounds,
        U: ArgBounds,
    {
        /// Convenience function for constructing from a string of [`crate::clap::ModifyCmd`] lines
        ///
        /// # Errors
        /// Returns an error if parsing a command or applying the command fails
        ///
        /// # Example
        /// ```
        /// use bucket_spigot::{Network, path::Path};
        /// let network: Network<String, String> = Network::from_commands_str(
        ///     "
        ///     add-joint .
        ///     add-bucket .0
        ///     set-filters .0.0 filter values
        ///     fill-bucket .0.0 item1 item2 item3
        ///     "
        /// ).unwrap();
        ///
        /// let bucket_path: Path = ".0.0".parse().unwrap();
        /// let bucket_path = bucket_path.as_ref();
        ///
        /// let expected_filters = vec!["filter".to_string(), "values".to_string()];
        /// assert_eq!(network.get_filters(bucket_path).unwrap(), &[&expected_filters]);
        /// ```
        // NOTE: Separate from `std::string::FromStr`, because a "string of commands" is not a canonical representation
        pub fn from_commands_str(commands: &str) -> Result<Self, Error> {
            let mut network = Self::default();
            network.modify_with_commands_str(commands)?;
            Ok(network)
        }
        /// Modifies the network according to a string of [`crate::clap::ModifyCmd`] lines
        ///
        /// # Errors
        /// Returns an error if parsing a command or applying the command fails
        pub fn modify_with_commands_str(&mut self, commands: &str) -> Result<(), Error> {
            use clap::Parser as _;

            #[derive(clap::Parser)]
            #[clap(no_binary_name = true)]
            struct Command<T, U>
            where
                T: ArgBounds,
                U: ArgBounds,
            {
                #[clap(subcommand)]
                inner: super::ModifyCmd<T, U>,
            }

            for (index, line) in commands.lines().enumerate() {
                let cmd = line.trim();
                if cmd.is_empty() || cmd.starts_with('#') {
                    continue;
                }

                let make_error = |kind| Error {
                    line: cmd.to_owned(),
                    line_number: index + 1,
                    kind,
                };

                let cmd = Command::<T, U>::try_parse_from(cmd.split_whitespace())
                    .map_err(ErrorKind::Parse)
                    .map_err(make_error)?;

                self.modify(cmd.inner.into())
                    .map_err(ErrorKind::Modify)
                    .map_err(make_error)?;
            }
            Ok(())
        }
    }
    // TODO when test can pass (fill-bucket and set-filter is included in serialize commands)
    // impl<T, U> Network<T, U>
    // where
    //     T: crate::clap::ArgBounds + serde::Serialize,
    //     U: crate::clap::ArgBounds,
    // {
    //     /// Convenience function for serializing a [`Network`] structure as [`crate::clap::ModifyCmd`] lines
    //     ///
    //     /// # Errors
    //     /// Returns an error if parsing a command or applying the command fails
    //     ///
    //     /// # Example
    //     /// ```
    //     /// use bucket_spigot::{Network, path::Path};
    //     /// let construction_string = "add-joint .
    //     /// add-bucket .0
    //     /// set-filters .0.0 filter values
    //     /// fill-bucket .0.0 item1 item2 item3";
    //     /// let network: Network<String, String> =
    //     ///     Network::from_commands_str(construction_string).unwrap();
    //     /// let command_lines = network.as_command_lines();
    //     /// assert_eq!(command_lines, construction_string);
    //     /// ```
    //     // NOTE: Separate from `std::string::FromStr`, because a "string of commands" is not a canonical representation
    //     #[must_use]
    //     pub fn as_command_lines(&self) -> String {
    //         self.serialize_as_command_lines()
    //             .into_iter()
    //             .fold(String::new(), |mut buf, line| {
    //                 use std::fmt::Write as _;
    //                 if !buf.is_empty() {
    //                     writeln!(buf).expect("string format is infallible");
    //                 }
    //                 write!(buf, "{line}").expect("string format is infallible");
    //                 buf
    //             })
    //     }
    // }

    /// Failure to modify a [`Network`] by script commands
    #[derive(Debug)]
    pub struct Error {
        line: String,
        line_number: usize,
        kind: ErrorKind,
    }
    #[derive(Debug)]
    enum ErrorKind {
        Parse(::clap::Error),
        Modify(crate::ModifyError),
    }
    impl std::error::Error for Error {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            match &self.kind {
                ErrorKind::Parse(error) => Some(error),
                ErrorKind::Modify(error) => Some(error),
            }
        }
    }
    impl std::fmt::Display for Error {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let Self {
                line,
                line_number,
                kind,
            } = self;
            let description = match kind {
                ErrorKind::Parse(_) => "parsing",
                ErrorKind::Modify(_) => "modify",
            };
            write!(
                f,
                "{description} command failed on line {line_number}: {line:?}"
            )
        }
    }
}
