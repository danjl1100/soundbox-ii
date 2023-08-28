// Copyright (C) 2021-2023  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use crate::{
    persistence::{FromKdlEntries, KdlEntryVistor, NodeErrorKind, ParseError, SequencerConfig},
    SequencerTree,
};

#[derive(Clone, Debug)]
struct NoOpFilter;
impl FromKdlEntries for NoOpFilter {
    type Error = String;
    type Visitor = NoOpFilterVisitor;
    fn try_finish(_: Self::Visitor) -> Result<Self, Self::Error> {
        Ok(Self)
    }
}
#[derive(Default)]
struct NoOpFilterVisitor;
impl NoOpFilterVisitor {
    fn check_fail_condition(key: &str, value: std::borrow::Cow<'_, str>) -> Result<(), String> {
        if key == "fail" {
            Err(value.into_owned())
        } else {
            Ok(())
        }
    }
}
impl KdlEntryVistor for NoOpFilterVisitor {
    type Error = String;
    fn visit_entry_str(&mut self, key: &str, value: &str) -> Result<(), Self::Error> {
        Self::check_fail_condition(key, value.into())
    }
    fn visit_entry_i64(&mut self, key: &str, value: i64) -> Result<(), Self::Error> {
        Self::check_fail_condition(key, format!("{value}").into())
    }
    fn visit_entry_bool(&mut self, key: &str, value: bool) -> Result<(), Self::Error> {
        Self::check_fail_condition(key, format!("{value}").into())
    }
    fn visit_value_str(&mut self, _value: &str) -> Result<(), Self::Error> {
        Ok(())
    }
    fn visit_value_i64(&mut self, _value: i64) -> Result<(), Self::Error> {
        Ok(())
    }
    fn visit_value_bool(&mut self, _value: bool) -> Result<(), Self::Error> {
        Ok(())
    }
}
type ConfigAndTree<F> = (SequencerConfig<(), F>, SequencerTree<(), F>);
fn from_str_no_op_filter(s: &str) -> Result<ConfigAndTree<NoOpFilter>, ParseError<NoOpFilter>> {
    SequencerConfig::parse_from_str(s)
}

#[test]
fn empty() {
    let empty = "root {}";
    let (config, seq_tree) = from_str_no_op_filter(empty).expect("valid seq KDL");
    assert_eq!(config.previous_doc.to_string(), empty);

    let tree = seq_tree.tree;
    assert_eq!(tree.sum_node_count(), 1);
}

#[test]
fn simple() {
    let inputs = [
        (
            "root {
                leaf {}
            }",
            2,
        ),
        ("root { chain {}; }", 2),
        ("root { chain { leaf; }; }", 3),
        ("root { chain { chain; }; }", 3),
    ];
    for (input, expected_count) in inputs {
        let (config, seq_tree) = from_str_no_op_filter(input).expect("valid seq KDL");
        assert_eq!(config.previous_doc.to_string(), input);

        let tree = seq_tree.tree;
        assert_eq!(tree.sum_node_count(), expected_count);
    }
}

#[test]
fn attribute_types_valid() {
    let simple = r#"root str="12345" bool=true i64=-3409432493"#;
    let (config, seq_tree) = from_str_no_op_filter(simple).expect("valid seq KDL");
    assert_eq!(config.previous_doc.to_string(), simple);

    let tree = seq_tree.tree;
    assert_eq!(tree.sum_node_count(), 1);
}

#[test]
fn weights() {
    let weights = "root {
        leaf weight=2     /* leaf 1 */
        leaf weight=3     /* leaf 2 */
        chain weight=1 {  /* chain 3 */
            leaf weight=0 /* leaf 4 */
        }
    }";
    let (config, seq_tree) = from_str_no_op_filter(weights).expect("valid seq KDL");
    assert_eq!(config.previous_doc.to_string(), weights);

    let tree = seq_tree.tree;
    assert_eq!(tree.sum_node_count(), 5);

    let path_root = tree.root_id().into_inner();
    let path_leaf1 = path_root.append(0);
    let path_leaf2 = path_root.append(1);
    let path_chain3 = path_root.append(2);
    let path_leaf4 = path_chain3.clone().append(0);

    let (leaf1_weight, _) = path_leaf1.try_ref_shared(&tree).expect("leaf1 exists");
    assert_eq!(leaf1_weight, 2);
    let (leaf2_weight, _) = path_leaf2.try_ref_shared(&tree).expect("leaf2 exists");
    assert_eq!(leaf2_weight, 3);
    let (chain3_weight, _) = path_chain3.try_ref_shared(&tree).expect("chain3 exists");
    assert_eq!(chain3_weight, 1);
    let (leaf4_weight, _) = path_leaf4.try_ref_shared(&tree).expect("leaf4 exists");
    assert_eq!(leaf4_weight, 0);
}

#[test]
fn error_root_missing() {
    let no_input = "";
    let Err(err) = from_str_no_op_filter(no_input) else {
        panic!("expected error")
    };
    let ParseError::Node(node_err) = err else {
        panic!("expected ParseError, got {err:?}")
    };
    assert_eq!(node_err.kind, NodeErrorKind::RootMissing);
}
#[test]
fn error_root_tag_invalid() {
    let input = "not-root";
    let Err(err) = from_str_no_op_filter(input) else {
        panic!("expected error")
    };
    let ParseError::Node(node_err) = err else {
        panic!("expected ParseError, got {err:?}")
    };
    assert_eq!(
        node_err.kind,
        NodeErrorKind::TagNameInvalid {
            found: "not-root".to_string(),
            expected: &["root"],
        }
    );
}
#[test]
fn error_root_duplicate() {
    let input = r#"root {
    }
    another {}"#;
    let Err(err) = from_str_no_op_filter(input) else {
        panic!("expected error")
    };
    let ParseError::Node(node_err) = err else {
        panic!("expected ParseError, got {err:?}")
    };
    assert_eq!(node_err.kind, NodeErrorKind::RootDuplicate);
}
#[test]
fn error_root_weight() {
    let input = r#"root weight=1 {}"#;
    let Err(err) = from_str_no_op_filter(input) else {
        panic!("expected error")
    };
    let ParseError::Node(node_err) = err else {
        panic!("expected ParseError, got {err:?}")
    };
    assert_eq!(node_err.kind, NodeErrorKind::RootWeight);
}
#[test]
fn error_attribute_invalid() {
    let attr_invalid = r#"root fail="fail1""#;
    let Err(err) = from_str_no_op_filter(attr_invalid) else {
        panic!("expected error")
    };
    let ParseError::Node(node_err) = err else {
        panic!("expected ParseError, got {err:?}")
    };
    assert_eq!(
        node_err.kind,
        NodeErrorKind::AttributesInvalid("fail1".to_string())
    );
}
#[test]
fn error_attribute_invalid_type() {
    let tests = [
        "root 0b01010110",
        "root 0xFACEB01D",
        "root 0o247210",
        "root 2.34",
        "root null",
    ];
    for attr_invalid in tests {
        let Err(err) = from_str_no_op_filter(attr_invalid) else {
            panic!("expected error")
        };
        let ParseError::Node(node_err) = err else {
            panic!("expected ParseError, got {err:?}")
        };
        assert_eq!(node_err.kind, NodeErrorKind::AttributeInvalidType);
    }
}
#[test]
fn error_attribute_invalid_passthru() {
    let tests = [
        (r#"root fail="this-fail""#, "this-fail"),
        (r#"root fail="this-fail" fail="not this""#, "this-fail"),
        (r#"root dk=true dk=true dk=true fail="goose""#, "goose"),
        (r#"root "a" "b" "c" 1 2 3 fail="x""#, "x"),
    ];
    for (attr_invalid, fail_arg) in tests {
        let Err(err) = from_str_no_op_filter(attr_invalid) else {
            panic!("expected error")
        };
        let ParseError::Node(node_err) = err else {
            panic!("expected ParseError, got {err:?}")
        };
        assert_eq!(
            node_err.kind,
            NodeErrorKind::AttributesInvalid(fail_arg.to_string())
        );
    }
}
#[test]
fn error_weight_invalid() {
    #[rustfmt::skip]
    let tests = [
        (NodeErrorKind::WeightInvalidType, "root { chain weight=0b01010110; }"),
        (NodeErrorKind::WeightInvalidType, "root { chain weight=0xFACEB01D; }"),
        (NodeErrorKind::WeightInvalidType, "root { chain weight=0o247210; }"),
        (NodeErrorKind::WeightInvalidType, "root { chain weight=2.34; }"),
        (NodeErrorKind::WeightInvalidType, "root { chain weight=null; }"),
        (NodeErrorKind::WeightInvalidType, "root { chain weight=\"string\"; }"),
        (NodeErrorKind::WeightInvalidType, "root { chain weight=true; }"),
        (NodeErrorKind::WeightInvalidValue, "root { chain weight=4294967296; }"),
    ];
    for (expected_kind, weight_invalid) in tests {
        let Err(err) = from_str_no_op_filter(weight_invalid) else {
            panic!("expected error")
        };
        let ParseError::Node(node_err) = err else {
            panic!("expected ParseError, got {err:?}")
        };
        assert_eq!(node_err.kind, expected_kind);
    }
}
#[test]
fn error_weight_duplicate() {
    let weight_duplicate = "root { chain weight=1 weight=5; }";
    let Err(err) = from_str_no_op_filter(weight_duplicate) else {
        panic!("expected error")
    };
    let ParseError::Node(node_err) = err else {
        panic!("expected ParseError, got {err:?}")
    };
    let NodeErrorKind::WeightDuplicate { first, second } = node_err.kind else {
        panic!("expected WeightDuplicate, got {node_err:?}");
    };
    assert_eq!(first.0, 1);
    assert_eq!(second.0, 5);
}
#[test]
fn error_leaf_not_empty() {
    let input = "root { leaf { leaf; }; }";
    let Err(err) = from_str_no_op_filter(input) else {
        panic!("expected error")
    };
    let ParseError::Node(node_err) = err else {
        panic!("expected ParseError, got {err:?}")
    };
    assert_eq!(node_err.kind, NodeErrorKind::LeafNotEmpty);
}
#[test]
fn error_leaf_tag_invalid() {
    let input = "root { not-chain-or-leaf; }";
    let Err(err) = from_str_no_op_filter(input) else {
        panic!("expected error")
    };
    let ParseError::Node(node_err) = err else {
        panic!("expected ParseError, got {err:?}")
    };
    assert_eq!(
        node_err.kind,
        NodeErrorKind::TagNameInvalid {
            found: "not-chain-or-leaf".to_string(),
            expected: &["chain", "leaf"],
        }
    );
}
#[test]
fn error_chain_tag_invalid() {
    let input = "root { not-chain { leaf; }; }";
    let Err(err) = from_str_no_op_filter(input) else {
        panic!("expected error")
    };
    let ParseError::Node(node_err) = err else {
        panic!("expected ParseError, got {err:?}")
    };
    assert_eq!(
        node_err.kind,
        NodeErrorKind::TagNameInvalid {
            found: "not-chain".to_string(),
            expected: &["chain"],
        }
    );
}
