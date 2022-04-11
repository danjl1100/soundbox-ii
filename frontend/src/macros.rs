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
                    yew::services::ConsoleService::$type(&format!( $dol($arg),+ ));
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

macro_rules! set_detect_change {
    (debug; $( $self:ident . $target:ident = $source:expr ;)+) => {
        {
            $(
                debug!("{:?} => {:?}  changed? {:?}", &$self.$target, &$source, $self.$target != $source);
            )+
            set_detect_change! {
                $( $self . $target = $source ;)+
            }
        }
    };
    ($( $self:ident . $target:ident = $source:expr ;)+) => {
        {
            let changed = $(
                ( $self.$target != $source )
            )||+;
            $(
                $self.$target = $source;
            )+
            changed
        }
    };
}

macro_rules! derive_wrapper {
    (
        $(
            $(#[$meta:meta])*
            enum $name:ident for $target:ident {
                $(
                    $variant:ident ( $inner:ty ) for $update_fn:ident (..)
                ),+ $(,)?
            }
        )+
    ) => {
        $(
            $(#[$meta])*
            enum $name {
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
                fn update_on(self, target: &mut $target) -> ShouldRender {
                    match self {
                        $($name::$variant(inner) => target.$update_fn(inner)),+
                    }
                }
            }
        )+
    }
}
