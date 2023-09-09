// Copyright (C) 2021-2023  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Adds, parses, updates, and removed internal-only annotations for [`KdlNode`]s

use kdl::KdlNode;
use std::num::ParseIntError;

/// Removes the leading `/*seq:0;*/`
pub(crate) fn strip_leading_seq(doc_node: &mut KdlNode) {
    let leading_update = doc_node.leading().and_then(|leading| {
        SeqAnnotation::try_new(leading).and_then(|a| a.after_annotation().map(String::from))
    });
    doc_node.set_leading(leading_update.unwrap_or_default());

    if let Some(doc_children) = doc_node.children_mut() {
        for doc_child in doc_children.nodes_mut() {
            strip_leading_seq(doc_child);
        }
    }
}

/// Adds the sequence annotation to the specified node
/// NOTE: Assumes there are no existing annotations (for optimization)
pub(crate) fn add_seq_to_vanilla_node(doc_node: &mut KdlNode, sequence: u64) {
    let new_leading = doc_node.leading().map_or_else(
        || SeqAnnotation::fmt_new(sequence),
        |old_leading| SeqAnnotation::fmt_new_before(old_leading, sequence),
    );
    doc_node.set_leading(new_leading);
}

pub(crate) fn get_seq(doc_node: &KdlNode) -> Option<Result<u64, ParseIntError>> {
    let leading = doc_node.leading()?;
    let annotation = SeqAnnotation::try_new(leading)?;
    Some(annotation.parse_seq())
}

pub(crate) fn update_seq(doc_node: &mut KdlNode, seq_new: u64) {
    let leading_update = if let Some(leading) = doc_node.leading() {
        if let Some(annotation) = SeqAnnotation::try_new(leading) {
            let seq_old = annotation
                .parse_seq()
                .expect("valid sequence added during parse/update");
            if seq_old == seq_new {
                // nothing
                None
            } else {
                Some(annotation.format_overwrite_value(seq_new))
            }
        } else {
            Some(SeqAnnotation::fmt_new_before(leading, seq_new))
        }
    } else {
        Some(SeqAnnotation::fmt_new(seq_new))
    };
    if let Some(leading_update) = leading_update {
        doc_node.set_leading(leading_update);
    }
}
struct SeqAnnotation<'a> {
    full_text: &'a str,
    prefix_start: usize,
    value_start: usize,
    suffix_start: usize,
    suffix_end: usize,
}
impl<'a> SeqAnnotation<'a> {
    const PREFIX: &str = "/*seq=";
    const SUFFIX: &str = "*/";

    pub fn try_new(full_text: &'a str) -> Option<Self> {
        if full_text.starts_with(Self::PREFIX) {
            let prefix_start = 0;
            let value_start = prefix_start + Self::PREFIX.len();
            let suffix_start = full_text.find(Self::SUFFIX)?;
            let suffix_end = suffix_start + Self::SUFFIX.len();
            Some(Self {
                full_text,
                prefix_start,
                value_start,
                suffix_start,
                suffix_end,
            })
        } else {
            None
        }
    }
    fn seq_str(&self) -> &str {
        let Self {
            value_start,
            suffix_start,
            ..
        } = *self;
        &self.full_text[value_start..suffix_start]
    }
    pub fn parse_seq(&self) -> Result<u64, ParseIntError> {
        self.seq_str().parse()
    }
    pub fn format_overwrite_value(self, seq: u64) -> String {
        let Self {
            full_text,
            prefix_start,
            value_start: _,
            suffix_start: _,
            suffix_end,
        } = self;
        let before_prefix = &full_text[0..prefix_start];
        let prefix = Self::PREFIX;
        let value = seq;
        let suffix = Self::SUFFIX;
        let after_suffix = &full_text[suffix_end..];
        format!("{before_prefix}{prefix}{value}{suffix}{after_suffix}")
    }
    pub fn after_annotation(&self) -> Option<&str> {
        let Self { suffix_end, .. } = *self;
        (suffix_end < self.full_text.len()).then_some(&self.full_text[suffix_end..])
    }
}
impl SeqAnnotation<'_> {
    pub fn fmt_new_before(existing: &str, seq: u64) -> String {
        format!(
            "{prefix}{seq}{suffix}{existing}",
            prefix = Self::PREFIX,
            suffix = Self::SUFFIX
        )
    }
    pub fn fmt_new(seq: u64) -> String {
        Self::fmt_new_before("", seq)
    }
}
