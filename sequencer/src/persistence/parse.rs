// Copyright (C) 2021-2023  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Parses the user-editable configuration of a [`SequencerTree`]

use super::{FromKdlEntries, KdlEntryVisitor, NodeError, NodeErrorKind, SingleRootKdlDocument};
use crate::{SequencerTree, SequencerTreeGuard};
use kdl::{KdlDocument, KdlNode};
use q_filter_tree::{
    id::{ty, NodePath, NodePathRefTyped},
    Weight,
};

const EXPECTED_NAME_ROOT: &[&str] = &[super::NAME_ROOT];
const EXPECTED_NAMES_CHAIN: &[&str] = &[super::NAME_CHAIN];
const EXPECTED_NAMES_CHAIN_LEAF: &[&str] = &[super::NAME_CHAIN, super::NAME_LEAF];

const ATTRIBUTE_WEIGHT: &str = "weight";
const DEFAULT_WEIGHT: Weight = 1;

struct Parser<'a, T, F>
where
    T: Clone,
{
    seq_tree_guard: SequencerTreeGuard<'a, T, F>,
}
pub(super) fn parse_nodes<T, F>(
    doc: KdlDocument,
) -> Result<(SingleRootKdlDocument, SequencerTree<T, F>), NodeError<F::Error>>
where
    T: Clone,
    F: FromKdlEntries,
{
    let doc = SingleRootKdlDocument::try_from(doc).map_err(|(err, doc)| NodeError {
        span: *doc.span(),
        kind: NodeErrorKind::RootCount(err),
    })?;
    let root = doc.single_root();

    if root.name().value() != super::NAME_ROOT {
        return Err(NodeError::tag_name_expected(&root, EXPECTED_NAME_ROOT));
    }

    let (root_weight, root_filter) = entries_to_weight_and_filter(root)?;
    if let Some((_, span)) = root_weight {
        return Err(NodeError {
            span,
            kind: NodeErrorKind::RootWeight,
        });
    }

    let mut seq_tree = SequencerTree::new(root_filter);
    let root_path = seq_tree.tree.root_id().into_inner();
    let seq_tree_guard = seq_tree.guard();
    Parser { seq_tree_guard }.parse(root, root_path)?;

    Ok((doc, seq_tree))
}
impl<T, F> Parser<'_, T, F>
where
    T: Clone,
    F: FromKdlEntries,
{
    fn parse(
        mut self,
        root: &KdlNode,
        root_path: NodePath<ty::Root>,
    ) -> Result<(), NodeError<F::Error>> {
        let root_path = (&root_path).into();
        self.add_nodes(root_path, node_children(root))
    }
    /// Adds all of the specified children to the tree, underneath the specified path prefix
    fn add_nodes(
        &mut self,
        parent_path: NodePathRefTyped<'_>,
        nodes: &[KdlNode],
    ) -> Result<(), NodeError<F::Error>> {
        const EXPECT_VALID_PARENT_PATH: &str = "valid parent_path upon construction";

        for node in nodes {
            let (weight_opt, filter) = entries_to_weight_and_filter(node)?;
            let weight = weight_opt.map_or(DEFAULT_WEIGHT, |(weight, _span)| weight);

            let children = node_children(node);

            let new_node_id = match node.name().value() {
                n if n == super::NAME_CHAIN => {
                    // chain = chain node (may or may not be empty)
                    Ok(self
                        .seq_tree_guard
                        .add_node(parent_path, filter)
                        .expect(EXPECT_VALID_PARENT_PATH))
                }
                n if n == super::NAME_LEAF => {
                    // leaf = empty terminal node
                    children
                        .is_empty()
                        .then(|| {
                            self.seq_tree_guard
                                .add_terminal_node(parent_path, filter)
                                .expect(EXPECT_VALID_PARENT_PATH)
                        })
                        .ok_or(NodeError {
                            span: *node.span(),
                            kind: NodeErrorKind::LeafNotEmpty,
                        })
                }
                _ => {
                    // invalid name
                    let expected_names = if children.is_empty() {
                        EXPECTED_NAMES_CHAIN_LEAF
                    } else {
                        EXPECTED_NAMES_CHAIN
                    };
                    Err(NodeError::tag_name_expected(node, expected_names))
                }
            }?;
            let mut new_node = new_node_id
                .try_ref(&mut self.seq_tree_guard.guard)
                .expect("created node path exists");
            new_node.set_weight(weight);

            let new_node_path = new_node_id.into_inner();
            let new_node_path = (&new_node_path).into();

            if !children.is_empty() {
                // chain node
                self.add_nodes(new_node_path, children)?;
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
            Some(name) if name.value() == ATTRIBUTE_WEIGHT => {
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

fn node_children(node: &KdlNode) -> &[KdlNode] {
    node.children().map(KdlDocument::nodes).unwrap_or_default()
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
