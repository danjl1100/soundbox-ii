// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! [`clap`] compatible versions of types

use crate::path::Path;

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

#[cfg(test)]
impl<T, U> crate::ModifyCmd<T, U>
where
    T: ArgBounds,
    U: ArgBounds,
{
    pub(crate) fn display_as_cmd(&self) -> impl std::fmt::Display + '_ {
        use crate::ModifyCmd as Other;
        use clap_crate::ValueEnum as _;

        struct Ret<'a, T, U>(&'a Other<T, U>)
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
                        let new_order_type = OrderType::from(*new_order_type)
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
