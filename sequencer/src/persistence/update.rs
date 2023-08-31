// Copyright (C) 2021-2023  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Updates the user-editable configuration of a [`SequencerTree`]

use super::{IntoKdlEntries, SingleRootKdlDocument};
use crate::SequencerTree;
use kdl::KdlNode;
use q_filter_tree::Weight;
use shared::Never;

pub(super) fn update_for_nodes<T, F>(
    doc: Option<SingleRootKdlDocument>,
    sequencer_tree: &SequencerTree<T, F>,
) -> (SingleRootKdlDocument, Result<(), F::Error<Never>>)
where
    T: Clone,
    F: IntoKdlEntries,
{
    let mut doc = doc.unwrap_or_default();
    let doc_root = doc.single_root_mut();

    // TODO pass the iterator.. down?
    // !! OR just query each node's children (similar to doc_node update structure)
    let (_, tree_root) = sequencer_tree.tree.enumerate().next().expect("root exists");

    let update_result = update_node(doc_root, None, tree_root);

    (doc, update_result)
}

fn update_node<T, F>(
    doc_node: &mut KdlNode,
    weight: Option<Weight>,
    tree_node: &q_filter_tree::Node<T, F>,
) -> Result<(), F::Error<Never>>
where
    F: IntoKdlEntries,
{
    let filter = &tree_node.filter;

    visitor::with_attribute_update_visitor(doc_node, |visitor| {
        filter.try_into_kdl(visitor).map(|_| ())
    })?;

    // TODO test for adding AND REMOVING child nodes to match tree_node

    Ok(())
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

            // assert_eq!(self.node.entries().len(), self.entries_checklist.len());
            // let next = self
            //     .node
            //     .entries_mut()
            //     .iter_mut()
            //     .zip(self.entries_checklist.iter_mut())
            //     .find(|(entry, checklist)| {
            //         checklist.is_some() && entry.name().map_or(false, |name| name.value() == key)
            //     });

            // if let Some((next_entry, check_entry)) = next {
            //     let result = f(next_entry.value_mut());

            //     check_entry.take();

            //     result
            // } else {
            //     let mut new_value = KdlValue::Null;
            //     let result = f(&mut new_value);

            //     let new_entry = KdlEntry::new_prop(key, new_value);
            //     self.node.entries_mut().push(new_entry);

            //     self.entries_checklist.push(None);

            //     result
            // }
        }
        fn with_argument_value<T>(&mut self, f: impl FnOnce(&mut KdlValue) -> T) -> T {
            self.with_entry_generic(f, |name| name.is_none(), KdlEntry::new)

            // assert_eq!(self.node.entries().len(), self.entries_checklist.len());
            // let next = self
            //     .node
            //     .entries_mut()
            //     .iter_mut()
            //     .zip(self.entries_checklist.iter_mut())
            //     .find(|(entry, checklist)| checklist.is_some() && entry.name().is_none());

            // if let Some((next_entry, check_entry)) = next {
            //     let result = f(next_entry.value_mut());

            //     check_entry.take();

            //     result
            // } else {
            //     let mut new_value = KdlValue::Null;
            //     let result = f(&mut new_value);

            //     let new_entry = KdlEntry::new(new_value);
            //     self.node.entries_mut().push(new_entry);

            //     self.entries_checklist.push(None);

            //     result
            // }
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
        // /// Returns the first unused property with the specified key (or creates one if needed)
        // fn find_property_value(&mut self, key: &str) -> &mut KdlValue {
        //     todo!()
        // }
        // /// Returns the first unused argument (or creates one if needed)
        // fn next_argument_value(&mut self) -> &mut KdlValue {
        //     let next = self
        //         .node
        //         .entries_mut()
        //         .iter_mut()
        //         .zip(self.entries_checklist.iter_mut())
        //         .find(|(entry, checklist)| checklist.is_some() && entry.name().is_none());
        //     if next.is_none() {
        //         self.node.entries_mut().push(KdlEntry::new(value));
        //     }
        // }
    }
    fn set_value_str(doc_value: &mut KdlValue, value: &str) {
        match doc_value {
            KdlValue::RawString(doc_str) | KdlValue::String(doc_str) if doc_str == value => {
                // nothing
            }
            KdlValue::RawString(doc_str) => {
                // TODO test for keeping raw strings raw
                *doc_str = value.to_string();
            }
            doc_value => {
                // TODO test for this smartness, promoting to RawString if value contains quotes
                *doc_value = if value.contains('"') {
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
