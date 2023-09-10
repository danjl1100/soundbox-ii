// Copyright (C) 2021-2023  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Parses the user-editable configuration of a [`SequencerTree`]

use super::{
    single_root::{EmptyKdlDocument, KdlNodesBySequence},
    FromKdlEntries, KdlEntryVisitor, NodeError, NodeErrorKind, SingleRootKdlDocument,
};
use crate::{SequencerTree, SequencerTreeGuard};
use kdl::{KdlDocument, KdlNode};
use q_filter_tree::{
    id::{ty, NodeId, NodeIdRefTyped, NodeIdTyped, SequenceSource},
    Weight,
};

const EXPECTED_NAME_ROOT: &[&str] = &[super::NAME_ROOT];
const EXPECTED_NAME_CHAIN: &[&str] = &[super::NAME_CHAIN];
const EXPECTED_NAME_LEAF: &[&str] = &[super::NAME_LEAF];

type DocAndTree<T, F> = (KdlNodesBySequence, SequencerTree<T, F>);

struct Parser<'a, T, F>
where
    T: Clone,
{
    seq_tree_guard: SequencerTreeGuard<'a, T, F>,
    doc_nodes_flat: KdlNodesBySequence,
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
    let (doc_empty, doc_root) = doc.into_parts_remove_root();

    if doc_root.name().value() != super::NAME_ROOT {
        return Err(NodeError::tag_name_expected(&doc_root, EXPECTED_NAME_ROOT));
    }

    let (root_weight, root_filter) = entries_to_weight_and_filter(&doc_root)?;
    if let Some((_, span)) = root_weight {
        return Err(NodeError {
            span,
            kind: NodeErrorKind::RootWeight,
        });
    }

    let mut seq_tree = SequencerTree::new(root_filter);
    let root_id = seq_tree.tree.root_id();
    let seq_tree_guard = seq_tree.guard();
    let doc_flat = Parser::new(seq_tree_guard, doc_empty).parse(doc_root, root_id)?;

    Ok((doc_flat, seq_tree))
}
impl<'a, T, F> Parser<'a, T, F>
where
    T: Clone,
    F: FromKdlEntries,
{
    fn new(seq_tree_guard: SequencerTreeGuard<'a, T, F>, doc_empty: EmptyKdlDocument) -> Self {
        Self {
            seq_tree_guard,
            doc_nodes_flat: KdlNodesBySequence::new(doc_empty),
        }
    }
    fn parse(
        mut self,
        src_doc_root: KdlNode,
        root_id: NodeId<ty::Root>,
    ) -> Result<KdlNodesBySequence, NodeError<F::Error>> {
        if src_doc_root.children().is_none() {
            return Err(NodeError {
                span: *src_doc_root.span(),
                kind: NodeErrorKind::TagMissingChildBlock,
            });
        }

        self.add_node((&root_id).into(), src_doc_root)?;

        Ok(self.doc_nodes_flat)
    }
    /// Adds the specified document node to the tree, underneath the specified path prefix
    fn add_node(
        &mut self,
        tree_id: NodeIdRefTyped<'_>,
        mut src_doc_node: KdlNode,
    ) -> Result<(), NodeError<F::Error>> {
        const EXPECT_VALID_PARENT_PATH: &str = "valid parent_path upon construction";

        for src_doc_child in src_doc_node
            .children_mut()
            .as_mut()
            .map_or(&mut vec![], KdlDocument::nodes_mut)
            .drain(..)
        {
            let (weight_opt, filter) = entries_to_weight_and_filter(&src_doc_child)?;
            let weight = weight_opt.map_or(super::DEFAULT_WEIGHT, |(weight, _span)| weight);

            let new_node_id = match src_doc_child.name().value() {
                n if n == super::NAME_CHAIN => {
                    // chain = chain node (may or may not be empty)
                    // but MUST have a child block, even if empty
                    if src_doc_child.children().is_some() {
                        Ok(self
                            .seq_tree_guard
                            .add_node(tree_id.into(), filter)
                            .expect(EXPECT_VALID_PARENT_PATH))
                    } else {
                        Err(NodeError {
                            span: *src_doc_child.span(),
                            kind: NodeErrorKind::TagMissingChildBlock,
                        })
                    }
                }
                n if n == super::NAME_LEAF => {
                    // leaf = empty terminal node
                    src_doc_child
                        .children()
                        .is_none()
                        .then(|| {
                            self.seq_tree_guard
                                .add_terminal_node(tree_id.into(), filter)
                                .expect(EXPECT_VALID_PARENT_PATH)
                        })
                        .ok_or(NodeError {
                            span: *src_doc_child.span(),
                            kind: NodeErrorKind::LeafNotEmpty,
                        })
                }
                _ => {
                    // invalid name
                    let expected_names = if src_doc_child.children().is_some() {
                        EXPECTED_NAME_CHAIN
                    } else {
                        EXPECTED_NAME_LEAF
                    };
                    Err(NodeError::tag_name_expected(&src_doc_child, expected_names))
                }
            }?;
            let mut new_node = new_node_id
                .try_ref(&mut self.seq_tree_guard.guard)
                .expect("created node path exists");
            new_node.set_weight(weight);

            self.add_node((&NodeIdTyped::from(new_node_id)).into(), src_doc_child)?;
        }

        let seq = tree_id.sequence();
        let existing = self.doc_nodes_flat.insert(src_doc_node, seq);
        assert!(
            existing.is_none(),
            "duplicate node for sequence {seq}: {existing:?}"
        );

        Ok(())
    }
}

type WeightAndSpan = (Weight, miette::SourceSpan);
fn entries_to_weight_and_filter<F: FromKdlEntries>(
    src_doc_node: &KdlNode,
) -> Result<(Option<WeightAndSpan>, F), NodeError<F::Error>> {
    let mut visitor = F::Visitor::default();
    let mut weight = None;

    for entry in src_doc_node.entries() {
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
        span: *src_doc_node.span(),
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
