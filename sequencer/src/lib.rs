// soundbox-ii/sequencer music playback controller *don't keep your sounds boxed up*
// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.
//
//! Sequences tracks from various sources

// TODO: only while building
#![allow(dead_code)]
// teach me
#![deny(clippy::pedantic)]
// no unsafe
#![forbid(unsafe_code)]
// no unwrap
#![deny(clippy::unwrap_used)]
// no panic
#![deny(clippy::panic)]
// docs!
#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

use std::{borrow::Cow, iter};

use q_filter_tree::{
    error::InvalidNodePath,
    id::{ty, NodeId, NodeIdTyped, NodePathTyped},
    iter::IterDetachedNodeMut,
    serde::{NodeDescriptor, NodeIdParseError},
    RemoveError,
};

use sources::ItemSource;
pub mod sources;

// conversions, for ergonomic use with `ItemSource`
type Item<T> = <T as ItemSource>::Item;
type Arg<T> = <T as ItemSource>::Arg;
type ItemError<T> = <T as ItemSource>::Error;
type Tree<T> = q_filter_tree::Tree<Item<T>, Arg<T>>;
type NodeInfo<T> = q_filter_tree::NodeInfo<Item<T>, Arg<T>>;

/// Sequencer for tracks (using [`q_filter_tree`] back-end) from a user-specified source
#[derive(Default)]
pub struct Sequencer<T: ItemSource> {
    tree: Tree<T>,
    item_source: T,
}
impl<T: ItemSource> Sequencer<T> {
    /// Creates a new, empty Sequencer
    pub fn new(item_source: T) -> Self {
        Self {
            tree: q_filter_tree::Tree::new(),
            item_source,
        }
    }
    /// Returns a mutable reference to the inner item source
    //TODO - should this be `pub`?  (e.g. is this a valid use-case outside of tests?)
    fn ref_item_source(&mut self) -> &mut T {
        &mut self.item_source
    }
    fn inner_add_node(&mut self, parent_path_str: &str) -> Result<NodeId<ty::Child>, Error> {
        let parent_path = parse_path(parent_path_str)?;
        let mut parent_ref = parent_path.try_ref(&mut self.tree)?;
        let mut child_nodes = parent_ref
            .child_nodes()
            .ok_or_else(|| format!("Node {parent_path} does not have child_nodes"))?;
        let new_node_id = child_nodes.add_child_default();
        Ok(new_node_id)
    }
    /// Adds a `Node` to the specified path.
    /// Returns the path of the created node.
    ///
    /// # Errors
    /// Returns an [`Error`] when inputs do not match the inner tree state
    pub fn add_node(&mut self, parent_path_str: &str, filter: Arg<T>) -> Result<String, Error> {
        let new_node_id = self.inner_add_node(parent_path_str)?;
        let mut node_ref = new_node_id.try_ref(&mut self.tree)?;
        node_ref.filter = filter;
        Ok(serialize_id(NodeIdTyped::from(new_node_id))?)
    }
    /// Adds a terminal `Node` to the specified path.
    /// Returns the path of the created node.
    ///
    /// # Errors
    /// Returns an [`Error`] when inputs do not match the inner tree state
    pub fn add_terminal_node(
        &mut self,
        node_path_str: &str,
        filter: Arg<T>,
    ) -> Result<String, Error> {
        let new_node_id = self.inner_add_node(node_path_str)?;
        let mut node_ref = new_node_id.try_ref(&mut self.tree)?;
        node_ref.filter = filter;
        node_ref.overwrite_child_items_uniform(iter::empty());
        Self::inner_update_node(
            &mut self.item_source,
            self.tree
                .enumerate_mut_subtree(&new_node_id)
                .expect("created node exists"),
        )?;
        Ok(serialize_id(new_node_id)?)
    }
    /// Updates the items for the specified node (and any children)
    ///
    /// # Errors
    /// Returns an [`Error`] when inputs do not match the inner tree state
    pub fn update_node(&mut self, node_path_str: &str) -> Result<String, Error> {
        let node_path = parse_path(node_path_str)?;
        let iter = self.tree.enumerate_mut_subtree(&node_path)?;
        Self::inner_update_node(&mut self.item_source, iter)?;
        Ok(serialize_path(node_path)?)
    }
    fn inner_update_node(
        item_source: &mut T,
        mut iter: IterDetachedNodeMut<'_, Item<T>, Arg<T>>,
    ) -> Result<(), Error> {
        iter.with_all(|args, _path, mut node_ref| {
            let is_items = node_ref.child_nodes().is_none();
            if is_items {
                let items = item_source
                    .lookup(args)
                    .map_err(|e| format!("item lookup error: {e}"))?;
                node_ref.merge_child_items_uniform(items);
            }
            Ok(())
        })
    }
    /// Removes a `Node` at the specified id (path`#`sequence)
    ///
    /// # Errors
    /// Returns an [`Error`] when inputs do not match the inner tree state
    pub fn remove_node(
        &mut self,
        node_id_str: &str,
    ) -> Result<(q_filter_tree::Weight, NodeInfo<T>), Error> {
        let node_id = match parse_id(node_id_str)? {
            NodeIdTyped::Root(..) => Err(Error::Message("cannot remove root node".to_string())),
            NodeIdTyped::Child(child) => Ok(child),
        }?;
        Ok(self.tree.remove_node(&node_id)??)
    }
    /// Returns the next [`Item`](`ItemSource::Item`)
    pub fn pop_next(&mut self) -> Option<Cow<'_, Item<T>>> {
        self.tree.pop_item()
    }
}

fn serialize_id<T: Into<NodeIdTyped>>(id: T) -> Result<String, serde_json::Error> {
    serde_json::to_string(&id.into())
}
fn serialize_path<T: Into<NodePathTyped>>(path: T) -> Result<String, serde_json::Error> {
    serde_json::to_string(&path.into())
}
fn parse_path(input_str: &str) -> Result<NodePathTyped, String> {
    match parse_descriptor(input_str)? {
        NodeDescriptor::Path(node_path) => Ok(node_path),
        NodeDescriptor::Id(node_id) => {
            eprint!("coerced id \"{node_id}\" ");
            let node_path = node_id.into();
            eprintln!("into path \"{node_path}\"");
            Ok(node_path)
        }
    }
}
fn parse_id(input_str: &str) -> Result<NodeIdTyped, String> {
    parse_descriptor(input_str)?
        .try_into()
        .map_err(|node_path| {
            format!("expected NodeId, got {node_path:?}. Try adding #id. (e.g. {input_str}#ID)")
        })
}
fn parse_descriptor(input_str: &str) -> Result<NodeDescriptor, String> {
    input_str
        .parse()
        .map_err(|e: NodeIdParseError| e.to_string())
}

impl<T: ItemSource> std::fmt::Display for Sequencer<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let tree = serde_json::to_string_pretty(&self.tree)
            .unwrap_or_else(|err| format!("<<error: {err}>>"));
        write!(f, "Sequencer {tree}")
    }
}

shared::wrapper_enum! {
    /// Error generated by [`Sequencer`] commands
    #[derive(Debug)]
    pub enum Error {
        /// Custom message
        Message(String),
        /// Serialization error
        Serde(serde_json::Error),
        /// Invalid [`NodePath`] sent
        InvalidNodePath(InvalidNodePath),
        /// Node removal error
        RemoveError(RemoveError),
    }
}

/// [`ItemSource`] used for debugging
#[derive(Default)]
pub struct DebugItemSource;
impl ItemSource for DebugItemSource {
    type Arg = String;
    type Item = String;
    type Error = shared::Never;

    fn lookup(&self, args: &[Self::Arg]) -> Result<Vec<Self::Item>, Self::Error> {
        let debug_label = format!("{args:?}");
        Ok((0..10)
            .map(|n| format!("item # {n} for {}", &debug_label))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use std::{borrow::Cow, collections::VecDeque};

    use q_filter_tree::OrderType;

    use crate::{DebugItemSource, Error, ItemSource, Sequencer};

    #[derive(Default)]
    struct UpdateTrackingItemSource(u32);
    impl UpdateTrackingItemSource {
        fn set_rev(&mut self, rev: u32) {
            self.0 = rev;
        }
    }
    impl ItemSource for UpdateTrackingItemSource {
        type Arg = String;
        type Item = String;
        type Error = shared::Never;

        fn lookup(&self, args: &[Self::Arg]) -> Result<Vec<Self::Item>, Self::Error> {
            let rev = self.0;
            let debug_label = format!("{args:?} rev {rev}");
            Ok((0..10)
                .map(|n| format!("item # {n} for {}", &debug_label))
                .collect())
        }
    }

    #[test]
    fn create_item_node() -> Result<(), Error> {
        let filename = "filename1.txt";

        let mut s = Sequencer::new(DebugItemSource);
        s.add_terminal_node(".", filename.to_string())?;
        assert_eq!(
            s.pop_next(),
            Some(Cow::Borrowed(&format!(
                "item # 0 for {:?}",
                vec!["", filename]
            )))
        );

        Ok(())
    }

    #[test]
    fn remove_node() -> Result<(), Error> {
        let mut s = Sequencer::new(DebugItemSource);
        assert_eq!(s.tree.sum_node_count(), 1, "beginning length");
        // add
        s.add_node(".", "".to_string())?;
        assert_eq!(s.tree.sum_node_count(), 2, "length after add");
        // remove
        let expect_removed = q_filter_tree::NodeInfo::Chain {
            queue: VecDeque::new(),
            filter: String::new(),
            order: OrderType::default(),
        };
        assert_eq!(s.remove_node(".0#1")?, (1, expect_removed));
        assert_eq!(s.tree.sum_node_count(), 1, "length after removal");
        Ok(())
    }

    fn assert_next(
        sequencer: &mut Sequencer<UpdateTrackingItemSource>,
        filters: &[&str],
        sequence: usize,
        rev: usize,
    ) {
        assert_eq!(
            sequencer.pop_next(),
            Some(Cow::Borrowed(&format!(
                "item # {sequence} for {filters:?} rev {rev}"
            )))
        );
    }
    #[test]
    fn update_node() -> Result<(), Error> {
        let filename = "foo_bar_file";

        let mut s = Sequencer::new(UpdateTrackingItemSource(0));
        s.add_terminal_node(".", filename.to_string())?;
        let filters = vec!["", filename];
        assert_next(&mut s, &filters, 0, 0);
        assert_next(&mut s, &filters, 1, 0);
        assert_next(&mut s, &filters, 2, 0);
        //
        s.ref_item_source().set_rev(52);
        assert_next(&mut s, &filters, 3, 0);
        assert_next(&mut s, &filters, 4, 0);
        assert_next(&mut s, &filters, 5, 0);
        s.update_node(".")?;
        assert_next(&mut s, &filters, 6, 52);
        assert_next(&mut s, &filters, 7, 52);
        assert_next(&mut s, &filters, 8, 52);
        Ok(())
    }
    #[test]
    fn update_subtree() -> Result<(), Error> {
        let mut s = Sequencer::new(UpdateTrackingItemSource(0));
        s.add_node(".", "base1".to_string())?;
        s.add_terminal_node(".0", "child1".to_string())?;
        s.add_terminal_node(".0", "child2".to_string())?;
        s.add_node(".", "base2".to_string())?;
        s.add_terminal_node(".1", "child3".to_string())?;
        let filters_child1 = vec!["", "base1", "child1"];
        let filters_child2 = vec!["", "base1", "child2"];
        let filters_child3 = vec!["", "base2", "child3"];
        //
        assert_next(&mut s, &filters_child1, 0, 0);
        assert_next(&mut s, &filters_child3, 0, 0);
        assert_next(&mut s, &filters_child2, 0, 0);
        assert_next(&mut s, &filters_child3, 1, 0);
        //
        s.ref_item_source().set_rev(5);
        assert_next(&mut s, &filters_child1, 1, 0);
        assert_next(&mut s, &filters_child3, 2, 0);
        assert_next(&mut s, &filters_child2, 1, 0);
        assert_next(&mut s, &filters_child3, 3, 0);
        s.update_node(".1.0")?;
        assert_next(&mut s, &filters_child1, 2, 0);
        assert_next(&mut s, &filters_child3, 4, 5);
        assert_next(&mut s, &filters_child2, 2, 0);
        assert_next(&mut s, &filters_child3, 5, 5);
        //
        s.ref_item_source().set_rev(8);
        assert_next(&mut s, &filters_child1, 3, 0);
        assert_next(&mut s, &filters_child3, 6, 5);
        assert_next(&mut s, &filters_child2, 3, 0);
        assert_next(&mut s, &filters_child3, 7, 5);
        s.update_node(".1")?;
        assert_next(&mut s, &filters_child1, 4, 0);
        assert_next(&mut s, &filters_child3, 8, 8);
        assert_next(&mut s, &filters_child2, 4, 0);
        assert_next(&mut s, &filters_child3, 9, 8);
        //
        s.ref_item_source().set_rev(9);
        assert_next(&mut s, &filters_child1, 5, 0);
        assert_next(&mut s, &filters_child3, 0, 8);
        assert_next(&mut s, &filters_child2, 5, 0);
        assert_next(&mut s, &filters_child3, 1, 8);
        s.update_node(".0")?;
        assert_next(&mut s, &filters_child1, 6, 9);
        assert_next(&mut s, &filters_child3, 2, 8);
        assert_next(&mut s, &filters_child2, 6, 9);
        assert_next(&mut s, &filters_child3, 3, 8);
        Ok(())
    }
}
