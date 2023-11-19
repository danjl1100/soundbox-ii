// Copyright (C) 2021-2023  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use crate::persistence::{SequencerConfig, StructSerializeDeserialize};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
struct HypotheticalFilter {
    #[serde(rename = "filter", default, skip_serializing_if = "str::is_empty")]
    value: String,
    #[serde(default, skip_serializing_if = "u32_is_zero")]
    count: u32,
}

#[allow(clippy::trivially_copy_pass_by_ref)] // reference type required by serde derive
fn u32_is_zero(n: &u32) -> bool {
    *n == 0
}

impl StructSerializeDeserialize for HypotheticalFilter {}

const INPUT: &str = r#"
/* this doc is annotated.  author: me */
root filter="weight not allowed on root" {
    chain /* this top-level chain is just to organize things */ {
        chain weight=5 filter="artist1" count=2 {
            leaf filter="year:2023"
        }
        leaf weight=2 filter="artist2"
    }
    /* trash can, to be restored if needed */
    chain weight=0 {
        chain {
            chain {
                chain {
                    chain {
                        /* well well well, someone was trying out deeply-nested nodes... */
                        leaf;
                    }
                }
                /* this is empty */
                leaf;
                chain {
                    /* also this one */
                    leaf;
                }
            }
        }
    }
}
"#;

#[test]
fn round_trip() {
    let (mut config, seq_tree) =
        SequencerConfig::<(), HypotheticalFilter>::parse_from_str(INPUT).expect("valid KDL");

    {
        let tree = &seq_tree.tree;
        // check "artist1" node
        let artist1_path = tree.root_id().append(0).append(0);
        let (weight, artist1_node) = artist1_path.try_ref_shared(tree).expect("node exists");
        assert_eq!(weight, 5);
        assert_eq!(
            artist1_node.filter,
            HypotheticalFilter {
                value: "artist1".to_string(),
                count: 2,
            }
        );
    }

    // complete round-trip
    let output = config
        .update_to_string(&seq_tree)
        .expect("re-serialize works");
    assert_eq!(INPUT, output);
}

mod error_messages_for_unimplemented {
    use super::{u32_is_zero, INPUT};
    use crate::persistence::{SequencerConfig, StructSerializeDeserialize};
    use serde::{Deserialize, Serialize};

    macro_rules! test_case {
        (
            $({
                for $type_name:expr;
                $(#[ $meta:meta ])*
                $field:ident : $ty:ty,
            })+
        ) => {{
            $(({
                #[derive(Clone, Debug, Serialize, Deserialize)]
                struct InvalidFilter {
                    #[serde(rename = "filter", default, skip_serializing_if = "str::is_empty")]
                    value: String,
                    #[serde(default, skip_serializing_if = "u32_is_zero")]
                    count: u32,
                    // ILLEGAL
                    $(#[$meta])*
                    $field: $ty,
                }
                impl StructSerializeDeserialize for InvalidFilter {}

                let (mut config, seq_tree) =
                    SequencerConfig::<(), InvalidFilter>::parse_from_str(INPUT).expect("valid KDL");

                let type_name = $type_name;
                let field = stringify!($field);

                let update_result = config.update_to_string(&seq_tree);
                if let Err(err) = &update_result {
                    let err_str = format!("{err}");
                    assert_eq!(
                        err_str,
                        format!(r#"serialize: serde-serialize to KDL is unimplemented for {type_name} (key "{field}")"#)
                    );
                    1 // <-- counter, to verify macro actually does things
                } else {
                    panic!("expected error for type_name {type_name:?}, field {field:?}");
                }
            })+)+ 0
        }};
    }

    #[test]
    fn unimplemented_std_type() {
        let cases_count_type = test_case! {
            {
                for r#"type "float""#;
                #[serde(default)]
                my_important_decimal: f32,
            }
            {
                for r#"type "float""#;
                #[serde(default)]
                my_important_decimal_precise: f64,
            }
            {
                for r#"type "char""#;
                #[serde(default)]
                favorite_letter: char,
            }
            // TODO unclear which type will trigger serde::Serializer::serialize_bytes
            // {
            //     for r#"type "bytes""#;
            //     some_data: (),
            // }
            {
                for r#"type "none""#;
                #[serde(default)]
                optional: Option<()>,
            }
            {
                for r#"type "some""#;
                #[serde(default="some_unit")]
                optional_2: Option<()>,
            }
            {
                for r#"type "unit""#;
                #[serde(default)]
                nothingness_in_a_package: (),
            }
            {
                for r#"type "seq""#;
                #[serde(default)]
                illegal_vec_field_name: Vec<()>,
            }
            {
                for r#"type "tuple""#;
                #[serde(default)]
                now_with_more_nothingness: ((), (), ()),
            }
            {
                for r#"type "map""#;
                #[serde(default)]
                treasure_map: std::collections::HashMap<u32, String>,
            }
        };
        // verify macro counter is as expected (avoid dead macro)
        assert_eq!(cases_count_type, 9);
    }

    #[test]
    fn unimplemented_user_type() {
        let cases_count_other = test_case! {
            {
                for r#"unit struct "UnitStruct""#;
                #[serde(default)]
                we_are_all_different: UnitStruct,
            }
            {
                for r#"unit variant "UnitEnum""#;
                #[serde(default)]
                we_are_all_unique: UnitEnum,
            }
            {
                for r#"newtype struct "NewtypeStruct""#;
                #[serde(default)]
                not_me: NewtypeStruct,
            }
            {
                for r#"newtype variant "NewtypeEnum""#;
                #[serde(default)]
                im_not_unique: NewtypeEnum,
            }
            {
                for r#"tuple struct "TupleStruct""#;
                #[serde(default)]
                and_the_crowd: TupleStruct,
            }
            {
                for r#"tuple variant "TupleEnum""#;
                #[serde(default)]
                goes: TupleEnum,
            }
            {
                for r#"struct variant "StructEnum""#;
                #[serde(default)]
                wild: StructEnum,
            }
        };
        // verify macro counter is as expected (avoid dead macro)
        assert_eq!(cases_count_other, 7);
    }

    #[allow(clippy::unnecessary_wraps)]
    fn some_unit() -> Option<()> {
        Some(())
    }

    #[derive(Clone, Debug, Serialize, Deserialize, Default)]
    struct UnitStruct;

    #[derive(Clone, Debug, Serialize, Deserialize, Default)]
    enum UnitEnum {
        #[default]
        Variant,
    }

    #[derive(Clone, Debug, Serialize, Deserialize, Default)]
    struct NewtypeStruct(UnitStruct);

    #[derive(Clone, Debug, Serialize, Deserialize)]
    enum NewtypeEnum {
        Variant(()),
    }
    impl Default for NewtypeEnum {
        fn default() -> Self {
            Self::Variant(())
        }
    }

    #[derive(Clone, Debug, Serialize, Deserialize, Default)]
    struct TupleStruct((), ());

    #[derive(Clone, Debug, Serialize, Deserialize)]
    enum TupleEnum {
        Variant((), ()),
    }
    impl Default for TupleEnum {
        fn default() -> Self {
            Self::Variant((), ())
        }
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    enum StructEnum {
        Variant {},
    }
    impl Default for StructEnum {
        fn default() -> Self {
            Self::Variant {}
        }
    }
}
