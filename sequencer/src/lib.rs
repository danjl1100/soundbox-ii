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

use std::borrow::Cow;

use q_filter_tree::{
    error::InvalidNodePath,
    id::{ty, NodeId, NodeIdTyped, NodePathTyped},
    iter::IterDetachedNodeMut,
    serde::{NodeDescriptor, NodeIdParseError},
    RemoveError,
};

/// Source of items for the [`Sequencer`]
pub trait ItemSource {
    /// Argument to the lookup, from each node in path to the terminal items node
    type Arg: serde::Serialize + Clone + Default;
    /// Element resulting from the lookup
    type Item: serde::Serialize + Clone;
    /// Retrieves [`Item`](`Self::Item`)s matching the specified [`Arg`](`Self::Arg`)s
    fn lookup(&self, args: &[Self::Arg]) -> Vec<Self::Item>;
}

// conversions, for ergonomic use with `ItemSource`
type Item<T> = <T as ItemSource>::Item;
type Arg<T> = <T as ItemSource>::Arg;
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
        filter_value: Arg<T>,
    ) -> Result<String, Error> {
        let new_node_id = self.inner_add_node(node_path_str)?;
        let mut node_ref = new_node_id.try_ref(&mut self.tree)?;
        node_ref.filter = filter_value;
        Self::inner_update_node(
            &mut self.item_source,
            self.tree
                .enumerate_mut_subtree(&new_node_id)
                .expect("created node exists"),
        );
        Ok(serialize_path(new_node_id)?)
    }
    // TODO
    // pub fn update_node(&mut self, node_path_str: &str) -> Result<(), Error> {
    //     let node_path = parse_path(node_path_str)?;
    //     let mut node_ref = node_path.try_ref(&mut self.tree)?;
    // }
    fn inner_update_node(item_source: &mut T, mut iter: IterDetachedNodeMut<'_, Item<T>, Arg<T>>) {
        iter.with_all(|args, _path, mut node_ref| {
            let items = item_source.lookup(args);
            node_ref.set_child_items_uniform(items);
        });
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

    fn lookup(&self, args: &[Self::Arg]) -> Vec<Self::Item> {
        let debug_label = format!("{args:?}");
        (0..10)
            .map(|n| format!("item # {n} for {}", &debug_label))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::{borrow::Cow, collections::VecDeque};

    use q_filter_tree::OrderType;

    use crate::{DebugItemSource, Error, ItemSource, Sequencer};

    #[derive(Default)]
    struct UpdateTrackingItemSource(u32);
    impl ItemSource for UpdateTrackingItemSource {
        type Arg = String;
        type Item = String;

        fn lookup(&self, args: &[Self::Arg]) -> Vec<Self::Item> {
            let rev = self.0;
            let debug_label = format!("{args:?} rev {rev}");
            (0..10)
                .map(|n| format!("item # {n} for {}", &debug_label))
                .collect()
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
        s.add_node(".")?;
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

    #[test]
    #[ignore]
    fn update_node() -> Result<(), Error> {
        todo!()
    }
}
