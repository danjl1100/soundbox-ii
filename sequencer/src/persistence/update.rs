// Copyright (C) 2021-2023  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Updates the user-editable configuration of a [`SequencerTree`]

use super::{single_root::KdlNodesBySequence, IntoKdlEntries, SingleRootKdlDocument};
use crate::SequencerTree;
use kdl::{KdlDocument, KdlEntry, KdlNode, KdlValue};
use q_filter_tree::Weight;
use shared::Never;

/// Updates the document for the current [`SequencerTree`] state, returning the [`KdlDocument`] string representation
pub(super) fn update_for_nodes<T, F>(
    flat_doc: Option<KdlNodesBySequence>,
    src_sequencer_tree: &SequencerTree<T, F>,
) -> (KdlNodesBySequence, Result<String, F::Error<Never>>)
where
    T: Clone,
    F: IntoKdlEntries,
{
    let flat_doc = flat_doc.unwrap_or_default();

    match Updater::new(flat_doc).update(src_sequencer_tree) {
        Ok(compiled_doc) => {
            // result we wanted - the STRING
            let string = compiled_doc.to_string();

            // deconstruct back into flattened document
            let (doc_empty, doc_root) = compiled_doc.into_parts_remove_root();
            let mut flat_doc = KdlNodesBySequence::new(doc_empty);

            return_nodes_to_flat(
                &mut flat_doc,
                doc_root,
                src_sequencer_tree.tree.root_node_shared(),
            );
            (flat_doc, Ok(string))
        }
        Err((flat_doc, err)) => (flat_doc, Err(err)),
    }
}

fn return_nodes_to_flat<T, F>(
    dest_flat_doc: &mut KdlNodesBySequence,
    mut src_doc_node: KdlNode,
    tree_node: &q_filter_tree::Node<T, F>,
) {
    if let Some((doc_children, tree_children)) = src_doc_node
        .children_mut()
        .as_mut()
        .zip(tree_node.child_nodes())
    {
        let mut doc_children = std::mem::take(doc_children);
        for (doc_child, (_weight, tree_child)) in
            doc_children.nodes_mut().drain(..).zip(tree_children)
        {
            return_nodes_to_flat(dest_flat_doc, doc_child, tree_child);
        }
    }

    let seq = tree_node.sequence_num();
    let existing = dest_flat_doc.insert(src_doc_node, seq);
    assert!(
        existing.is_none(),
        "duplicate nodes for seq {seq}: {existing:?}"
    );
}

struct Updater {
    src_flat_doc: KdlNodesBySequence,
}

impl Updater {
    fn new(src_flat_doc: KdlNodesBySequence) -> Self {
        Self { src_flat_doc }
    }
    fn update<T, F>(
        mut self,
        src_sequencer_tree: &SequencerTree<T, F>,
    ) -> Result<SingleRootKdlDocument, (KdlNodesBySequence, F::Error<Never>)>
    where
        T: Clone,
        F: IntoKdlEntries,
    {
        let tree_root = src_sequencer_tree.tree.root_node_shared();
        let root_seq = src_sequencer_tree.tree.root_id().sequence();

        let mut doc_root = self
            .src_flat_doc
            .try_remove(root_seq)
            .unwrap_or_else(|| KdlNode::new(super::NAME_ROOT));

        let update_result = self.update_node(&mut doc_root, None, tree_root);
        if let Err(err) = update_result {
            return_nodes_to_flat(&mut self.src_flat_doc, doc_root, tree_root);
            Err((self.src_flat_doc, err))
        } else {
            // add root node to the empty document
            let doc_empty = self.src_flat_doc.end_delete_remaining();
            let doc = doc_empty.with_root(doc_root);

            Ok(doc)
        }
    }
    fn update_node<T, F>(
        &mut self,
        dest_doc_node: &mut KdlNode,
        weight: Option<Weight>,
        src_tree_node: &q_filter_tree::Node<T, F>,
    ) -> Result<(), F::Error<Never>>
    where
        F: IntoKdlEntries,
    {
        update_filter(dest_doc_node, &src_tree_node.filter)?;

        if let Some(weight) = weight {
            update_weight(dest_doc_node, weight);
        }

        if let Some(tree_child_nodes) = src_tree_node.child_nodes() {
            let child_doc = dest_doc_node.children_mut();
            for (weight, tree_child) in tree_child_nodes {
                let doc_nodes = {
                    let child_doc = if let Some(child_doc) = child_doc.as_mut() {
                        child_doc
                    } else {
                        *child_doc = Some(KdlDocument::new());
                        child_doc.as_mut().expect("set to Some")
                    };
                    child_doc.nodes_mut()
                };
                //
                let needle_seq = tree_child.sequence_num();
                let existing_doc_child = self.src_flat_doc.try_remove(needle_seq);
                let mut doc_child = existing_doc_child.unwrap_or_else(|| {
                    let name = if tree_child.child_nodes().is_some() {
                        super::NAME_CHAIN
                    } else {
                        super::NAME_LEAF
                    };
                    KdlNode::new(name)
                });
                self.update_node(&mut doc_child, Some(weight), tree_child)?;
                doc_nodes.push(doc_child);
            }
        }

        Ok(())
    }
}

fn update_filter<F>(dest_doc_node: &mut KdlNode, filter: &F) -> Result<(), F::Error<Never>>
where
    F: IntoKdlEntries,
{
    visitor::with_attribute_update_visitor(dest_doc_node, |visitor| {
        filter.try_into_kdl(visitor).map(|_| ())
    })
}
fn update_weight(dest_doc_node: &mut KdlNode, weight: Weight) {
    let entries = dest_doc_node.entries_mut();
    // remove "weight" property if it exists
    let existing_weight_index = entries.iter().enumerate().find_map(|(index, entry)| {
        match entry.name() {
            Some(name) if name.value() == super::ATTRIBUTE_WEIGHT => true,
            _ => false, // not "weight" attribute"
        }
        .then_some(index)
    });
    let existing_weight_was_default = if let Some(existing_weight_index) = existing_weight_index {
        let current_value = entries
            .get(existing_weight_index)
            .expect("found index exists")
            .value()
            .as_i64();
        if current_value.map_or(false, |value| value == i64::from(weight)) {
            return;
        }
        let is_default =
            current_value.map_or(false, |value| value == i64::from(super::DEFAULT_WEIGHT));
        entries.remove(existing_weight_index);
        is_default
    } else {
        false
    };

    // add weight entry *FIRST*, if it is non-default
    if weight != super::DEFAULT_WEIGHT || existing_weight_was_default {
        let entry =
            KdlEntry::new_prop(super::ATTRIBUTE_WEIGHT, KdlValue::Base10(i64::from(weight)));
        entries.insert(0, entry);
    }
}

mod visitor {
    use crate::persistence::KdlEntryVisitor;
    use kdl::{KdlEntry, KdlIdentifier, KdlNode, KdlValue};
    use shared::Never;

    pub(super) fn with_attribute_update_visitor<T>(
        node: &mut KdlNode,
        f: impl FnOnce(AttributeUpdateVisitor) -> T,
    ) -> T {
        let visitor = AttributeUpdateVisitor::new(node);

        // TODO test for removing attributes no longer present

        f(visitor)
    }

    #[derive(Clone, Copy)]
    struct NeedVisit;

    pub(super) struct AttributeUpdateVisitor<'a> {
        node: &'a mut KdlNode,
        entries_checklist: Vec<Option<NeedVisit>>,
    }
    impl<'a> AttributeUpdateVisitor<'a> {
        fn new(node: &'a mut KdlNode) -> Self {
            let entries_checklist = vec![Some(NeedVisit); node.entries().len()];
            Self {
                node,
                entries_checklist,
            }
        }
        fn with_property_value<T>(&mut self, key: &str, f: impl FnOnce(&mut KdlValue) -> T) -> T {
            self.with_entry_generic(
                f,
                |name| name.map_or(false, |n| n.value() == key),
                |value| KdlEntry::new_prop(key, value),
            )
        }
        fn with_argument_value<T>(&mut self, f: impl FnOnce(&mut KdlValue) -> T) -> T {
            self.with_entry_generic(f, |name| name.is_none(), KdlEntry::new)
        }

        fn with_entry_generic<T>(
            &mut self,
            mutator_fn: impl FnOnce(&mut KdlValue) -> T,
            name_match_fn: impl Fn(Option<&KdlIdentifier>) -> bool,
            entry_generator_fn: impl FnOnce(KdlValue) -> KdlEntry,
        ) -> T {
            debug_assert!(self.node.entries().len() >= self.entries_checklist.len());

            let next = self
                .node
                .entries_mut()
                .iter_mut()
                .zip(self.entries_checklist.iter_mut())
                .find(|(entry, checklist)| checklist.is_some() && name_match_fn(entry.name()));

            if let Some((next_entry, check_entry)) = next {
                let result = mutator_fn(next_entry.value_mut());

                check_entry.take();

                result
            } else {
                let mut new_value = KdlValue::Null;
                let result = mutator_fn(&mut new_value);

                let new_entry = entry_generator_fn(new_value);
                self.node.entries_mut().push(new_entry);

                result
            }
        }
    }
    fn set_value_str(doc_value: &mut KdlValue, value: &str) {
        // reference: https://github.com/kdl-org/kdl-rs/blob/6044ef9776f24f45004c36d7628b1f5fbd83c8ad/src/value.rs#L256-L261
        const KDL_ESCAPED_CHARS: &[char] = &['\\', '"', '\n', '\r', '\t', '\u{08}', '\u{0C}'];

        match doc_value.as_string() {
            Some(doc_str) if doc_str == value => {
                // nothing
            }
            _ => {
                *doc_value = if value.contains(KDL_ESCAPED_CHARS) {
                    KdlValue::RawString(value.to_string())
                } else {
                    KdlValue::String(value.to_string())
                };
            }
        }
    }
    impl KdlEntryVisitor for AttributeUpdateVisitor<'_> {
        type Error = Never;

        fn visit_property_str(&mut self, key: &str, value: &str) -> Result<(), Self::Error> {
            self.with_property_value(key, |v| set_value_str(v, value));
            Ok(())
        }
        fn visit_property_i64(&mut self, key: &str, value: i64) -> Result<(), Self::Error> {
            self.with_property_value(key, |v| {
                *v = KdlValue::Base10(value);
            });
            Ok(())
        }
        fn visit_property_bool(&mut self, key: &str, value: bool) -> Result<(), Self::Error> {
            self.with_property_value(key, |v| {
                *v = KdlValue::Bool(value);
            });
            Ok(())
        }

        fn visit_argument_str(&mut self, value: &str) -> Result<(), Self::Error> {
            self.with_argument_value(|v| {
                set_value_str(v, value);
            });
            Ok(())
        }
        fn visit_argument_i64(&mut self, value: i64) -> Result<(), Self::Error> {
            self.with_argument_value(|v| {
                *v = KdlValue::Base10(value);
            });
            Ok(())
        }
        fn visit_argument_bool(&mut self, value: bool) -> Result<(), Self::Error> {
            self.with_argument_value(|v| {
                *v = KdlValue::Bool(value);
            });
            Ok(())
        }
    }
}
