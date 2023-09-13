// Copyright (C) 2021-2023  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use crate::persistence::{SequencerConfig, StructSerializeDeserialize};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
struct HypotheticalFilter {
    #[serde(rename = "filter", default, skip_serializing_if = "str::is_empty")]
    str: String,
    // TODO improve diagnostics, so the remedy for the error is clear
    #[serde(default)]
    v: Vec<()>, // ILLEGAL
}

impl StructSerializeDeserialize for HypotheticalFilter {}

#[test]
fn round_trip() {
    let input = r#"
/* this doc is annotated.  author: me */
root filter="weight not allowed on root" {
    chain /* this top-level chain is just to organize things */ {
        chain weight=5 filter="artist1" {
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
    let (mut config, seq_tree) =
        SequencerConfig::<(), HypotheticalFilter>::parse_from_str(input).expect("valid KDL");

    let output = config
        .update_to_string(&seq_tree)
        .expect("re-serialize works");
    assert_eq!(input, output);
    panic!("the discou")
}
