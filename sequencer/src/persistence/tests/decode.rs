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
// impl FromKdlEntries for NoOpFilter {
//     type Error = String;
//     fn try_from(
//         entries: &[kdl::KdlEntry],
//     ) -> Result<Self, (Self::Error, Option<miette::SourceSpan>)> {
//         if let Some((fail_str, span)) = entries.iter().find_map(|entry| {
//             if let Some(name) = entry.name() {
//                 if name.value() == "weight" {
//                     return Some(("weight not allowed", *entry.span()));
//                 }
//             }
//             match (entry.name(), entry.value()) {
//                 (Some(name), kdl::KdlValue::String(fail_str)) if name.value() == "fail" => {
//                     Some((fail_str, *entry.span()))
//                 }
//                 _ => None,
//             }
//         }) {
//             Err((fail_str.to_owned(), Some(span)))
//         } else {
//             Ok(NoOpFilter)
//         }
//     }
// }
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
    let simple = "root {
        leaf {}
    }";
    let (config, seq_tree) = from_str_no_op_filter(simple).expect("valid seq KDL");
    assert_eq!(config.previous_doc.to_string(), simple);

    let tree = seq_tree.tree;
    assert_eq!(tree.sum_node_count(), 2);
}

#[test]
fn weights() {
    let weights = "root {
        leaf weight=2     /* leaf 1 */
        leaf weight=3     /* leaf 2 */
        node weight=1 {   /* node 3 */
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
    let path_node3 = path_root.append(2);
    let path_leaf4 = path_node3.clone().append(0);

    let (leaf1_weight, _) = path_leaf1.try_ref_shared(&tree).expect("leaf1 exists");
    assert_eq!(leaf1_weight, 2);
    let (leaf2_weight, _) = path_leaf2.try_ref_shared(&tree).expect("leaf2 exists");
    assert_eq!(leaf2_weight, 3);
    let (node3_weight, _) = path_node3.try_ref_shared(&tree).expect("node3 exists");
    assert_eq!(node3_weight, 1);
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
    let no_input = "not-root";
    let Err(err) = from_str_no_op_filter(no_input) else {
        panic!("expected error")
    };
    let ParseError::Node(node_err) = err else {
        panic!("expected ParseError, got {err:?}")
    };
    assert_eq!(
        node_err.kind,
        NodeErrorKind::RootTagNameInvalid {
            found: "not-root".to_string(),
            expected: &["root"],
        }
    );
}
#[test]
fn error_root_duplicate() {
    let no_input = r#"root {
    }
    another {}"#;
    let Err(err) = from_str_no_op_filter(no_input) else {
        panic!("expected error")
    };
    let ParseError::Node(node_err) = err else {
        panic!("expected ParseError, got {err:?}")
    };
    assert_eq!(node_err.kind, NodeErrorKind::RootDuplicate);
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
