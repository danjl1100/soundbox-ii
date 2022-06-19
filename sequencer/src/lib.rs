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

use q_filter_tree::{
    error::InvalidNodePath,
    id::{ty, NodeId, NodeIdTyped, NodePathTyped},
    serde::{NodeDescriptor, NodeIdParseError},
    Tree,
};

/// Source of items for the [`Sequencer`]
pub trait ItemSource {
    /// Argument to the lookup
    type Arg: serde::Serialize + Clone; //TODO Why is `Clone` needed here??  (serde_json::to_string_pretty doesn't mention Clone...?!?!?!?)
    /// Element resulting from the lookup
    type Item: serde::Serialize + Clone; //TODO Why is `Clone` needed here??  (serde_json::to_string_pretty doesn't mention Clone...?!?!?!?)
    /// Retrieves [`Item`](`Self::Item`)s matching the specified [`Arg`](`Self::Arg`)s
    fn lookup(args: &[Option<Self::Arg>]) -> Vec<Self::Item>;
}

/// Sequencer for tracks (using [`q_filter_tree`] back-end) from text files
#[derive(Default)]
pub struct Sequencer<T: ItemSource> {
    tree: Tree<<T as ItemSource>::Item, <T as ItemSource>::Arg>,
    item_source: T,
}
impl<T: ItemSource> Sequencer<T> {
    /// Creates a new, empty Sequencer
    pub fn new(item_source: T) -> Self {
        Self {
            tree: Tree::new(),
            item_source,
        }
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
    pub fn add_node(&mut self, parent_path_str: &str) -> Result<String, Error> {
        let new_node_id = self.inner_add_node(parent_path_str)?;
        let new_node_path_str = serde_json::to_string(&NodePathTyped::from(new_node_id))?;
        Ok(new_node_path_str)
    }
    /// Adds a terminal `Node` to the specified path.
    /// Returns the path of the created node.
    ///
    /// # Errors
    /// Returns an [`Error`] when inputs do not match the inner tree state
    pub fn add_terminal_node(
        &mut self,
        node_path_str: &str,
        filename: <T as ItemSource>::Arg,
    ) -> Result<String, Error> {
        let new_node_id = self.inner_add_node(node_path_str)?;
        let mut node_ref = new_node_id.try_ref(&mut self.tree)?;
        node_ref.filter.replace(filename);
        // self.inner_update_node(node_ref)?;
        Ok(serialize_path(new_node_id)?)
    }
    // TODO
    // pub fn update_node(&mut self, node_path_str: &str) -> Result<(), Error> {
    //     let node_path = parse_path(node_path_str)?;
    //     let mut node_ref = node_path.try_ref(&mut self.tree)?;
    // }
    // fn inner_update_node(
    //     &mut self,
    //     node_ref: NodeRefMut<<T as ItemSource>::Item, <T as ItemSource>::Arg>,
    // ) -> Result<(), Error> {
    //     if let Some(child_nodes) = node_ref.child_nodes() {
    //         for child in child_nodes {
    //             let a: () = child;
    //             // child.set_child_items_uniform();
    //         }
    //     }
    //     node_ref.set_child_items_uniform()
    // }
    // TODO
    // pub fn remove_node(&mut self, node_id_str: &str, node_sequence: usize) -> Result<(), Error> {
    //     let node_path = parse_id(node_path_str)?;
    //     let node_id = node_path.with_sequence(id);
    // }
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
        .map_err(|node_path| format!("expected NodeId, got {node_path:?}. Try adding #id."))
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
    }
}

/// [`ItemSource`] used for debugging
#[derive(Default)]
pub struct DebugItemSource;
impl ItemSource for DebugItemSource {
    type Arg = String;
    type Item = String;

    fn lookup(args: &[Option<Self::Arg>]) -> Vec<Self::Item> {
        let debug_label = format!("{:?}", args);
        (0..10)
            .map(|n| format!("item # {} for {}", n, &debug_label))
            .collect()
    }
}
