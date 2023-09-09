// Copyright (C) 2021-2023  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Parses the user-editable configuration of a [`SequencerTree`]

use super::{
    annotate, FromKdlEntries, KdlEntryVisitor, NodeError, NodeErrorKind, SingleRootKdlDocument,
};
use crate::{SequencerTree, SequencerTreeGuard};
use kdl::{KdlDocument, KdlNode};
use q_filter_tree::{
    id::{ty, NodePath, NodePathRefTyped},
    Weight,
};

const EXPECTED_NAME_ROOT: &[&str] = &[super::NAME_ROOT];
const EXPECTED_NAME_CHAIN: &[&str] = &[super::NAME_CHAIN];
const EXPECTED_NAME_LEAF: &[&str] = &[super::NAME_LEAF];

type DocAndTree<T, F> = (SingleRootKdlDocument, SequencerTree<T, F>);

struct Parser<'a, T, F>
where
    T: Clone,
{
    seq_tree_guard: SequencerTreeGuard<'a, T, F>,
}
pub(super) fn parse_nodes<T, F>(doc: KdlDocument) -> Result<DocAndTree<T, F>, NodeError<F::Error>>
where
    T: Clone,
    F: FromKdlEntries,
{
    let doc = SingleRootKdlDocument::try_from(doc).map_err(|(err, doc)| NodeError {
        span: *doc.span(),
        kind: NodeErrorKind::RootCount(err),
    })?;
    let doc_root = doc.single_root();

    if doc_root.name().value() != super::NAME_ROOT {
        return Err(NodeError::tag_name_expected(doc_root, EXPECTED_NAME_ROOT));
    }

    let (root_weight, root_filter) = entries_to_weight_and_filter(doc_root)?;
    if let Some((_, span)) = root_weight {
        return Err(NodeError {
            span,
            kind: NodeErrorKind::RootWeight,
        });
    }

    let mut seq_tree = SequencerTree::new(root_filter);
    let root_path = seq_tree.tree.root_id().into_inner();
    let seq_tree_guard = seq_tree.guard();
    Parser { seq_tree_guard }.parse(doc_root, root_path)?;

    // mutate document, to annotate node ids
    let mut doc = doc;
    let root = seq_tree.tree.root_node_shared();
    annotate_node_ids(doc.single_root_mut(), root).expect("parsed nodes match doc nodes");

    Ok((doc, seq_tree))
}
impl<T, F> Parser<'_, T, F>
where
    T: Clone,
    F: FromKdlEntries,
{
    fn parse(
        mut self,
        doc_root: &KdlNode,
        root_path: NodePath<ty::Root>,
    ) -> Result<(), NodeError<F::Error>> {
        let root_path = (&root_path).into();
        let Some(doc_children) = doc_root.children() else {
            return Err(NodeError {
                span: *doc_root.span(),
                kind: NodeErrorKind::TagMissingChildBlock,
            });
        };
        self.add_nodes(root_path, doc_children.nodes())
    }
    /// Adds all of the specified children to the tree, underneath the specified path prefix
    fn add_nodes(
        &mut self,
        parent_path: NodePathRefTyped<'_>,
        doc_nodes: &[KdlNode],
    ) -> Result<(), NodeError<F::Error>> {
        const EXPECT_VALID_PARENT_PATH: &str = "valid parent_path upon construction";

        for doc_node in doc_nodes {
            let (weight_opt, filter) = entries_to_weight_and_filter(doc_node)?;
            let weight = weight_opt.map_or(super::DEFAULT_WEIGHT, |(weight, _span)| weight);

            let new_node_id = match doc_node.name().value() {
                n if n == super::NAME_CHAIN => {
                    // chain = chain node (may or may not be empty)
                    // but MUST have a child block, even if empty
                    if doc_node.children().is_some() {
                        Ok(self
                            .seq_tree_guard
                            .add_node(parent_path, filter)
                            .expect(EXPECT_VALID_PARENT_PATH))
                    } else {
                        Err(NodeError {
                            span: *doc_node.span(),
                            kind: NodeErrorKind::TagMissingChildBlock,
                        })
                    }
                }
                n if n == super::NAME_LEAF => {
                    // leaf = empty terminal node
                    doc_node
                        .children()
                        .is_none()
                        .then(|| {
                            self.seq_tree_guard
                                .add_terminal_node(parent_path, filter)
                                .expect(EXPECT_VALID_PARENT_PATH)
                        })
                        .ok_or(NodeError {
                            span: *doc_node.span(),
                            kind: NodeErrorKind::LeafNotEmpty,
                        })
                }
                _ => {
                    // invalid name
                    let expected_names = if doc_node.children().is_some() {
                        EXPECTED_NAME_CHAIN
                    } else {
                        EXPECTED_NAME_LEAF
                    };
                    Err(NodeError::tag_name_expected(doc_node, expected_names))
                }
            }?;
            let mut new_node = new_node_id
                .try_ref(&mut self.seq_tree_guard.guard)
                .expect("created node path exists");
            new_node.set_weight(weight);

            let new_node_path = new_node_id.into_inner();
            let new_node_path = (&new_node_path).into();

            if let Some(doc_children) = doc_node.children() {
                // chain node
                self.add_nodes(new_node_path, doc_children.nodes())?;
            }
        }
        Ok(())
    }
}

type WeightAndSpan = (Weight, miette::SourceSpan);
fn entries_to_weight_and_filter<F: FromKdlEntries>(
    node: &KdlNode,
) -> Result<(Option<WeightAndSpan>, F), NodeError<F::Error>> {
    let mut visitor = F::Visitor::default();
    let mut weight = None;

    for entry in node.entries() {
        let error_attribute_invalid = |err: F::Error| NodeError {
            span: *entry.span(),
            kind: NodeErrorKind::AttributesInvalid(err),
        };

        let error_attribute_type = || {
            Err(NodeError {
                span: *entry.span(),
                kind: NodeErrorKind::AttributeInvalidType,
            })
        };

        match entry.name() {
            Some(name) if name.value() == super::ATTRIBUTE_WEIGHT => {
                let kdl::KdlValue::Base10(new_weight) = entry.value() else {
                    return Err(NodeError {
                        span: *entry.span(),
                        kind: NodeErrorKind::WeightInvalidType,
                    });
                };
                let Ok(new_weight) = Weight::try_from(*new_weight) else {
                    return Err(NodeError {
                        span: *entry.span(),
                        kind: NodeErrorKind::WeightInvalidValue,
                    });
                };
                if let Some(first) = weight {
                    Err(NodeError {
                        span: *entry.span(),
                        kind: NodeErrorKind::WeightDuplicate {
                            first,
                            second: (new_weight, *entry.span()),
                        },
                    })
                } else {
                    weight.replace((new_weight, *entry.span()));
                    Ok(())
                }
            }
            Some(name) => {
                let key = name.value();
                match entry.value() {
                    kdl::KdlValue::RawString(value) | kdl::KdlValue::String(value) => visitor
                        .visit_property_str(key, value)
                        .map_err(error_attribute_invalid),
                    kdl::KdlValue::Base10(value) => visitor
                        .visit_property_i64(key, *value)
                        .map_err(error_attribute_invalid),
                    kdl::KdlValue::Bool(value) => visitor
                        .visit_property_bool(key, *value)
                        .map_err(error_attribute_invalid),
                    kdl::KdlValue::Base2(_)
                    | kdl::KdlValue::Base8(_)
                    | kdl::KdlValue::Base10Float(_) // NOTE: Explicitly disallowing floats.
                                                    // (can't think of a valid usecase for filters)
                    | kdl::KdlValue::Base16(_)
                    | kdl::KdlValue::Null => error_attribute_type(),
                }
            }
            None => match entry.value() {
                kdl::KdlValue::RawString(value) | kdl::KdlValue::String(value) => visitor
                    .visit_argument_str(value)
                    .map_err(error_attribute_invalid),
                kdl::KdlValue::Base10(value) => visitor
                    .visit_argument_i64(*value)
                    .map_err(error_attribute_invalid),
                kdl::KdlValue::Bool(value) => visitor
                    .visit_argument_bool(*value)
                    .map_err(error_attribute_invalid),
                kdl::KdlValue::Base2(_)
                | kdl::KdlValue::Base8(_)
                | kdl::KdlValue::Base10Float(_) // NOTE: Explicitly disallowing floats.
                                                // (can't think of a valid usecase for filters)
                | kdl::KdlValue::Base16(_)
                | kdl::KdlValue::Null => error_attribute_type(),
            },
        }?;
    }

    let filter = F::try_finish(visitor).map_err(|err| NodeError {
        span: *node.span(),
        kind: NodeErrorKind::AttributesInvalid(err),
    })?;
    Ok((weight, filter))
}

impl<E> NodeError<E> {
    fn tag_name_expected(node: &KdlNode, expected: &'static [&'static str]) -> Self {
        let node_name = node.name();
        let found = node_name.to_string();
        Self {
            span: *node_name.span(),
            kind: NodeErrorKind::TagNameInvalid { found, expected },
        }
    }
}

/// Recursively adds annotation comment to the [`KdlNode`]s with the sequence ID for each
/// corresponding [`q_filter_tree::Node`]
fn annotate_node_ids<T, F>(
    doc_node: &mut KdlNode,
    node: &q_filter_tree::Node<T, F>,
) -> Result<(), String> {
    let sequence = node.sequence_num();
    annotate::add_seq_to_vanilla_node(doc_node, sequence);

    match (doc_node.children_mut(), node.child_nodes()) {
        (Some(doc_children), Some(node_children)) => {
            let mut doc_children = doc_children.nodes_mut().iter_mut();
            for (_weight, tree_child) in node_children {
                let Some(doc_child) = doc_children.next() else {
                    return Err(format!(
                        "tree child with no matching doc_child: {tree_child:?}"
                    ));
                };
                annotate_node_ids(doc_child, tree_child)?;
            }
            if let Some(extra) = doc_children.next() {
                return Err(format!("extra doc child: {extra:?}"));
            }
        }
        (None, None) => {
            // nothing
        }
        (doc_children, node_children) => {
            let node_children = match node_children {
                None => "None".to_string(),
                Some(node_children) => {
                    let elems: Vec<_> = node_children.collect();
                    format!("{elems:?}")
                }
            };
            return Err(format!(
                "doc_children {doc_children:?} mismatch to node_children {node_children:?}"
            ));
        }
    }

    Ok(())
}
