// Copyright (C) 2021-2023  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Various utility macros for ergonomics in Yew

macro_rules! make_console {
    ($($type:ident),+) => {
        // hack to pass a literal `$` to the inner `macro_rules` invocation
        // ref: https://github.com/rust-lang/rust/issues/35853#issuecomment-443110660
        make_console!(@inner ($) $($type),+);
    };
    (@inner ($dol:tt) $($type:ident),+) => {
        $(
            #[allow(unused)]
            macro_rules! $type {
                ($dol($arg:expr),+ $dol(,)?) => {
                    gloo::console::$type!(format!( $dol($arg),+ ))
                };
            }
        )+
    };
}
make_console!(log, info, error, debug);

// TODO remove if not needed
// macro_rules! log_render {
//     ( $msg:expr ) => {
//         if $crate::LOG_RENDERS {
//             log!("RENDER {}", $msg);
//         }
//     };
// }

macro_rules! derive_wrapper {
    (
        $(
            $(#[$meta:meta])*
            $vis:vis enum $name:ident for $target:ident {
                $(
                    $variant:ident ( $inner:ty ) for $update_fn:ident (..)
                ),+ $(,)?
            }
        )+
    ) => {
        $(
            $(#[$meta])*
            $vis enum $name {
                $(
                    $variant ( $inner )
                ),+
            }
            $(
                impl From<$inner> for $name {
                    fn from(other: $inner) -> Self {
                        $name::$variant(other)
                    }
                }
            )+
            impl $name {
                fn update_on(self, target: &mut $target, ctx: &Context<$target>) -> bool {
                    match self {
                        $($name::$variant(inner) => target.$update_fn(ctx, inner)),+
                    }
                }
            }
        )+
    };
    (
        $(
            $(#[$meta:meta])*
            $vis:vis enum $name:ident for $Component:ident {
                $(
                    $variant:ident ( $inner_ty:ty ) for self.$update_member:ident
                ),+ $(,)?
            }
        )+
    ) => {
        $(
            $(#[$meta])*
            $vis enum $name {
                $(
                    $variant ( $inner_ty )
                ),+
            }
            $(
                impl From<$inner_ty> for $name {
                    fn from(other: $inner_ty) -> Self {
                        $name::$variant(other)
                    }
                }
            )+
            mod __sealed {
                use super::{$name, $Component};
                use $crate::macros::UpdateDelegate;
                pub(super) struct TickedAll { _sealed: () }
                impl $name {
                    pub(super) fn update_on(
                        self,
                        component: &mut $Component,
                        ctx: &yew::Context<$Component>,
                        _sentinel: TickedAll, // ensures the `tick_all` function is indeed called
                    ) -> bool {
                        match self {
                            $($name::$variant(inner_ty) => {
                                UpdateDelegate::<$Component>::update(&mut component.$update_member, ctx, inner_ty)
                            }),+
                        }
                    }
                    pub(super) fn tick_all(component: &mut $Component) -> TickedAll {
                        $(UpdateDelegate::<$Component>::tick_all(&mut component.$update_member);)+
                        TickedAll { _sealed: () }
                    }
                }
            }
        )+
    };
}
pub trait UpdateDelegate<C: yew::Component> {
    type Message;
    /// Update for the `Message` of interest
    fn update(&mut self, ctx: &yew::Context<C>, message: Self::Message) -> bool;
    /// Update on every message tick
    ///
    /// NOTE: This callback should only examine and update internal state,
    ///   not emit messages. (`yew::Context` is intentionally omitted)
    fn tick_all(&mut self) {}
}
