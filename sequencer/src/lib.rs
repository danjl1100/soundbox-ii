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

#[cfg(test)]
mod tests;

use sources::ItemSource;
pub mod sources;

// conversions, for ergonomic use with `ItemSource`
type Item<T, F> = <T as ItemSource<F>>::Item;
type ItemError<T, F> = <T as ItemSource<F>>::Error;
type Tree<T, F> = q_filter_tree::Tree<Item<T, F>, F>;
type NodeInfo<T, F> = q_filter_tree::NodeInfo<Item<T, F>, F>;

/// Sequencer for tracks (using [`q_filter_tree`] back-end) from a user-specified source
#[derive(Default)]
pub struct Sequencer<T: ItemSource<F>, F> {
    tree: Tree<T, F>,
    item_source: T,
}
impl<T: ItemSource<F>, F> Sequencer<T, F>
where
    F: Default + Clone,
{
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
    pub fn add_node(&mut self, parent_path_str: &str, filter: F) -> Result<String, Error> {
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
    pub fn add_terminal_node(&mut self, node_path_str: &str, filter: F) -> Result<String, Error> {
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
        mut iter: IterDetachedNodeMut<'_, Item<T, F>, F>,
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
    ) -> Result<(q_filter_tree::Weight, NodeInfo<T, F>), Error> {
        let node_id = match parse_id(node_id_str)? {
            NodeIdTyped::Root(..) => Err(Error::Message("cannot remove root node".to_string())),
            NodeIdTyped::Child(child) => Ok(child),
        }?;
        Ok(self.tree.remove_node(&node_id)??)
    }
    /// Returns the next [`Item`](`ItemSource::Item`)
    pub fn pop_next(&mut self) -> Option<Cow<'_, Item<T, F>>> {
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

impl<T: ItemSource<F>, F> std::fmt::Display for Sequencer<T, F>
where
    F: serde::Serialize + Clone,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let tree = serde_json::to_string_pretty(&self.tree)
            .unwrap_or_else(|err| format!("<<Sequencer error: {err}>>"));
        write!(f, "{tree}")
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
impl ItemSource<String> for DebugItemSource {
    type Item = String;
    type Error = shared::Never;

    fn lookup(&self, args: &[String]) -> Result<Vec<Self::Item>, Self::Error> {
        let debug_label = format!("{args:?}");
        Ok((0..10)
            .map(|n| format!("item # {n} for {}", &debug_label))
            .collect())
    }
}
