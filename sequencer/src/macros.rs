// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

macro_rules! command_enum {
    (
        $(#[$enum_meta:meta])*
        $enum_vis:vis enum $Enum:ident <'a, F>
        where
            F: Clone,
        {
            $(
                $(#[$struct_meta:meta])*
                $Struct:ident $(<$($struct_gen:tt),+>)? -> $struct_output:ident $(<$struct_outputF:ident>)? {
                    $(
                        $(#[$field_meta:meta])*
                        $field:ident : $field_ty:ty
                    ),* $(,)?
                }
            ),* $(,)?
        }
        $out_mod_vis:vis mod $out_mod:ident {
            $(#[$output_meta:meta])*
            $output_vis:vis enum $OutputEnum:ident <$mainF:ident> {
                $(
                    $(#[$output_meta_inner:meta])*
                    $output:ident $(<$outF:ident>)? ( $output_ty: ty )
                ),* $(,)?
            }
        }
    ) => {
        $(#[$enum_meta])*
        $enum_vis enum $Enum <'a, F> {
            $(
                $(#[$struct_meta])*
                $Struct( $Struct $(<$($struct_gen),+>)? )
            ),*
        }
        $(
            $(#[$struct_meta])*
            $enum_vis struct $Struct $(<$($struct_gen),+>)? {
                $(
                    $(#[$field_meta])*
                    pub $field : $field_ty
                ),*
            }
        )*
        $(
            impl<'a, F> From<$Struct $(<$($struct_gen),+>)? > for $Enum<'a, F> {
                fn from(other: $Struct $(<$($struct_gen),+>)? ) -> Self {
                    Self::$Struct(other)
                }
            }
        )*
        $out_mod_vis mod $out_mod {
            $(#[$output_meta])*
            #[derive(Clone, Debug, PartialEq, Eq)]
            $output_vis enum $OutputEnum <$mainF> {
                $(
                    $(#[$output_meta_inner])*
                    $output ( $output_ty )
                ),*
            }
        }
        fn _assert_run_outs<'a, F>(never: shared::Never, typed: $out_mod::$OutputEnum <$mainF> ) {
            fn _assert<T, F, O>(_: &shared::Never, _output: &O)
            where
                T: crate::command::Runnable<F, Output=O> {}
            $(
                match &typed {
                    $out_mod::$OutputEnum::$struct_output(output) =>
                        _assert::<$Struct $(<$($struct_gen),+>)?, F, _>(&never, &output),
                    _ => {}
                }
            )*
        }
        impl<'a, F> crate::command::Runnable<F> for $Enum <'a, F> {
            type Output = $out_mod::$OutputEnum<$mainF>;
            fn run<T>(self, sequencer: &mut Sequencer<T, F>) -> Result<Self::Output, Error>
            where
                T: ItemSource<F>,
                F: Clone
            {
                match self {
                    $(
                        Self::$Struct(inner) => inner
                            .run(sequencer)
                            .map($out_mod::$OutputEnum::$struct_output),
                    )+
                }
            }
        }
    };
}

macro_rules! command_runnable {
    (
        $(
            impl $(<$($gen:tt),+>)? Runnable<$F:ty> for $ty:ty {
                fn run($self:ident, $seq:ident) -> Result<$out:ty, Error> $block:block
            }
        )*
    ) => {
        $(
            impl $(<$($gen),+>)? crate::command::Runnable<$F> for $ty {
                type Output = $out;

                fn run<T>($self, $seq: &mut Sequencer<T, $F>) -> Result<Self::Output, Error>
                where
                    T: ItemSource<$F>,
                    $F: Clone,
                $block
            }
        )*
    };
}
