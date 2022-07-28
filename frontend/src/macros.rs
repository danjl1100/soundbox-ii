// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
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

macro_rules! log_render {
    ( $msg:expr ) => {
        if $crate::LOG_RENDERS {
            log!("RENDER {}", $msg);
        }
    };
}

// TODO remove if not needed in yew 0.19
// macro_rules! set_detect_change {
//     (debug; $( $self:ident . $target:ident = $source:expr ;)+) => {
//         {
//             $(
//                 debug!("{:?} => {:?}  changed? {:?}", &$self.$target, &$source, $self.$target != $source);
//             )+
//             set_detect_change! {
//                 $( $self . $target = $source ;)+
//             }
//         }
//     };
//     ($( $self:ident . $target:ident = $source:expr ;)+) => {
//         {
//             let changed = $(
//                 ( $self.$target != $source )
//             )||+;
//             $(
//                 $self.$target = $source;
//             )+
//             changed
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
            impl $name {
                fn update_on(self, component: &mut $Component, ctx: &Context<$Component>) -> bool {
                    use $crate::macros::UpdateDelegate;
                    match self {
                        $($name::$variant(inner_ty) => UpdateDelegate::<$Component>::update(&mut component.$update_member, ctx, inner_ty)),+
                    }
                }
            }
        )+
    }
}
pub trait UpdateDelegate<C: yew::Component> {
    type Message;
    fn update(&mut self, ctx: &yew::Context<C>, message: Self::Message) -> bool;
}
