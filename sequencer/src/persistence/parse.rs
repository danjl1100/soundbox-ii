// Copyright (C) 2021-2023  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Parses the user-editable configuration of a [`SequencerTree`]

use super::{FromKdlEntries, KdlEntryVistor, NodeError, NodeErrorKind};
use crate::{SequencerTree, SequencerTreeGuard};
use kdl::{KdlDocument, KdlNode};
use q_filter_tree::{
    id::{ty, NodePath, NodePathRefTyped},
    Weight,
};

const NAME_ROOT: &str = "root";
const NAME_CHAIN: &str = "chain";
const NAME_LEAF: &str = "leaf";

const EXPECTED_NAME_ROOT: &[&str] = &[NAME_ROOT];
const EXPECTED_NAMES_CHAIN_LEAF: &[&str] = &[NAME_CHAIN, NAME_LEAF];

const ATTRIBUTE_WEIGHT: &str = "weight";
const DEFAULT_WEIGHT: Weight = 1;

struct Parser<'a, T, F>
where
    T: Clone,
{
    seq_tree_guard: SequencerTreeGuard<'a, T, F>,
}
pub(super) fn parse_nodes<T, F>(
    doc: &KdlDocument,
) -> Result<SequencerTree<T, F>, NodeError<F::Error>>
where
    T: Clone,
    F: FromKdlEntries,
{
    let root = get_single_root(doc)?;

    if root.name().value() != NAME_ROOT {
        return Err(NodeError::tag_name_expected(&root, EXPECTED_NAME_ROOT));
    }

    let (root_weight, root_filter) = entries_to_weight_and_filter(root)?;
    if let Some((_, span)) = root_weight {
        // TODO test for this error
        // return Err(NodeError {
        //     span,
        //     kind: NodeErrorKind::RootWeight,
        // });
    }

    let mut seq_tree = SequencerTree::new(root_filter);
    let root_path = seq_tree.tree.root_id().into_inner();
    let seq_tree_guard = seq_tree.guard();
    Parser { seq_tree_guard }.parse(root, root_path)?;

    Ok(seq_tree)
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

            let new_node_id = if children.is_empty() {
                // terminal node
                // TODO verify KdlNode name is NAME_LEAF
                self.seq_tree_guard
                    .add_terminal_node(parent_path, filter)
                    .expect(EXPECT_VALID_PARENT_PATH)
            } else {
                // chain node
                // TODO verify KdlNode name is NAME_CHAIN
                self.seq_tree_guard
                    .add_node(parent_path, filter)
                    .expect(EXPECT_VALID_PARENT_PATH)
            };
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

fn get_single_root<E>(doc: &KdlDocument) -> Result<&KdlNode, NodeError<E>> {
    let mut nodes_iter = doc.nodes().iter();

    let Some(root) = nodes_iter.next() else {
        return Err(NodeError {
            span: *doc.span(),
            kind: NodeErrorKind::RootMissing,
        });
    };

    if let Some(extra) = nodes_iter.next() {
        return Err(NodeError {
            span: *extra.span(),
            kind: NodeErrorKind::RootDuplicate,
        });
    }
    debug_assert!(nodes_iter.next().is_none());

    Ok(root)
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
            return Ok(());
            // TODO test for this error
            Err(NodeError {
                span: *entry.span(),
                kind: NodeErrorKind::AttributeInvalidType,
            })
        };

        match entry.name() {
            Some(name) if name.value() == ATTRIBUTE_WEIGHT => {
                let kdl::KdlValue::Base10(new_weight) = entry.value() else {
                    continue;
                    // TODO test for this error
                    return Err(NodeError {
                        span: *entry.span(),
                        kind: NodeErrorKind::WeightInvalidType,
                    });
                };
                let Ok(new_weight) = Weight::try_from(*new_weight) else {
                    continue;
                    // TODO test for this error
                    return Err(NodeError {
                        span: *entry.span(),
                        kind: NodeErrorKind::WeightInvalidValue,
                    });
                };
                if let Some(first) = weight {
                    continue;
                    // TODO test for this error
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
                        .visit_entry_str(key, value)
                        .map_err(error_attribute_invalid),
                    kdl::KdlValue::Base10(value) => visitor
                        .visit_entry_i64(key, *value)
                        .map_err(error_attribute_invalid),
                    kdl::KdlValue::Bool(value) => visitor
                        .visit_entry_bool(key, *value)
                        .map_err(error_attribute_invalid),
                    kdl::KdlValue::Base2(_)
                    | kdl::KdlValue::Base8(_)
                    | kdl::KdlValue::Base10Float(_)
                    | kdl::KdlValue::Base16(_)
                    | kdl::KdlValue::Null => error_attribute_type(),
                }
            }
            None => match entry.value() {
                kdl::KdlValue::RawString(value) | kdl::KdlValue::String(value) => visitor
                    .visit_value_str(value)
                    .map_err(error_attribute_invalid),
                kdl::KdlValue::Base10(value) => visitor
                    .visit_value_i64(*value)
                    .map_err(error_attribute_invalid),
                kdl::KdlValue::Bool(value) => visitor
                    .visit_value_bool(*value)
                    .map_err(error_attribute_invalid),
                kdl::KdlValue::Base2(_)
                | kdl::KdlValue::Base8(_)
                | kdl::KdlValue::Base10Float(_)
                | kdl::KdlValue::Base16(_)
                | kdl::KdlValue::Null => error_attribute_type(),
            },
        }?;
    }

    // TODO test for this error, too
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
        let found = node.name().to_string();
        Self {
            span: *node.span(),
            kind: NodeErrorKind::RootTagNameInvalid { found, expected },
        }
    }
}
