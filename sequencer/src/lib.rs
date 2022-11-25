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
    id::{ty, NodeId, NodeIdTyped, NodePathRefTyped, NodePathTyped},
    serde::{NodeDescriptor, NodeIdParseError},
    OrderType, RemoveError, Weight,
};

use std::borrow::Cow;

#[macro_use]
mod macros;

mod iter;

#[cfg(test)]
mod tests;

use sources::{multi_select::Mismatch, ItemSource};
pub mod sources;

pub mod command;

// conversions, for ergonomic use with `ItemSource`
type Item<T, F> = <T as ItemSource<F>>::Item;
type SeqItem<T, F> = q_filter_tree::SequenceAndItem<Item<T, F>>;
type Tree<T, F> = q_filter_tree::Tree<Item<T, F>, F>;
type TreeGuard<'a, T, F> = q_filter_tree::TreeGuard<'a, Item<T, F>, F>;
type NodeInfo<T, F> = q_filter_tree::NodeInfo<Item<T, F>, F>;

/// Sequencer for tracks (using [`q_filter_tree`] back-end) from a user-specified source
#[derive(Default)]
pub struct Sequencer<T: ItemSource<F>, F> {
    raw_tree: Tree<T, F>,
    item_source: T,
}
impl<T: ItemSource<F>, F> Sequencer<T, F> {
    /// Creates a new, empty Sequencer
    pub fn new(item_source: T, root_filter: F) -> Self {
        Self {
            raw_tree: q_filter_tree::Tree::new_with_filter(root_filter),
            item_source,
        }
    }
    fn tree_guard(&mut self) -> TreeGuard<'_, T, F> {
        self.raw_tree.guard()
    }
    fn tree_ref(&self) -> &Tree<T, F> {
        &self.raw_tree
    }
}
impl<T: ItemSource<F>, F> Sequencer<T, F>
where
    F: Clone,
{
    fn inner_add_node(
        &mut self,
        parent_path_str: &str,
        filter: F,
    ) -> Result<NodeId<ty::Child>, Error> {
        let parent_path = parse_path(parent_path_str)?;
        let mut tree_guard = self.tree_guard();
        let mut parent_ref = parent_path.try_ref(&mut tree_guard)?;
        let mut child_nodes = parent_ref
            .child_nodes()
            .ok_or_else(|| format!("Node {parent_path} does not have child_nodes"))?;
        let new_node_id = child_nodes.add_child_filter(filter);
        Ok(new_node_id)
    }
    /// Adds a `Node` to the specified path.
    /// Returns the path of the created node.
    ///
    /// # Errors
    /// Returns an [`Error`] when inputs do not match the inner tree state
    fn add_node(&mut self, parent_path_str: &str, filter: F) -> Result<NodeIdStr, Error> {
        let new_node_id = self.inner_add_node(parent_path_str, filter)?;
        Ok(serialize_id(NodeIdTyped::from(new_node_id))?)
    }
    /// Adds a terminal `Node` to the specified path.
    /// Returns the path of the created node.
    ///
    /// # Errors
    /// Returns an [`Error`] when inputs do not match the inner tree state
    fn add_terminal_node(&mut self, parent_path_str: &str, filter: F) -> Result<NodeIdStr, Error> {
        let new_node_id = self.inner_add_node(parent_path_str, filter)?;
        let mut tree_guard = self.raw_tree.guard();
        let mut node_ref = new_node_id.try_ref(&mut tree_guard)?;
        node_ref.overwrite_child_items_uniform(std::iter::empty());
        Self::inner_update_node(&mut self.item_source, &new_node_id, &mut tree_guard)?;
        Ok(serialize_id(new_node_id)?)
    }
    /// Sets the filter of the specified node
    /// Returns the previous filter value.
    ///
    /// # Errors
    /// Returns an [`Error`] when inputs do not match the inner tree state
    fn set_node_filter(&mut self, node_path_str: &str, filter: F) -> Result<F, Error> {
        let node_path = parse_path(node_path_str)?;
        // set the filter
        let mut tree_guard = self.raw_tree.guard();
        let mut node_ref = node_path.try_ref(&mut tree_guard)?;
        let old_filter = std::mem::replace(&mut node_ref.filter, filter);
        // update node (recursively)
        Self::inner_update_node(&mut self.item_source, &node_path, &mut tree_guard)?;
        Ok(old_filter)
    }
    /// Sets the weight of the specified item in the node
    /// Returns the previous weight value.
    ///
    /// # Errors
    /// Returns an [`Error`] when inputs do not match the inner tree state
    fn set_node_item_weight(
        &mut self,
        node_path_str: &str,
        item_index: usize,
        weight: Weight,
    ) -> Result<Weight, Error> {
        let node_path = parse_path(node_path_str)?;
        let mut tree_guard = self.tree_guard();
        let mut node_ref = node_path.try_ref(&mut tree_guard)?;
        let child_items = node_ref.child_items().ok_or_else(|| {
            Error::Message(format!(
                "cannot set items for node at path {node_path}, type is chain"
            ))
        })?;
        let mut child_items = child_items.ref_mut();
        match child_items.set_weight(item_index, weight) {
            Ok(old_weight) => Ok(old_weight),
            Err(invalid_index) => Err(Error::InvalidItemIndex(node_path, invalid_index)),
        }
    }
    /// Sets the weight of the specified node
    /// Returns the previous weight value.
    ///
    /// # Errors
    /// Returns an [`Error`] when inputs do not match the inner tree state
    fn set_node_weight(&mut self, node_path_str: &str, weight: Weight) -> Result<Weight, Error> {
        let node_path = parse_path(node_path_str)?;
        let node_path = match node_path {
            NodePathTyped::Root(path) => Err(InvalidNodePath::from(path)),
            NodePathTyped::Child(path) => Ok(path),
        }?;
        let mut tree_guard = self.tree_guard();
        let mut node_ref = node_path.try_ref(&mut tree_guard)?;
        Ok(node_ref.set_weight(weight))
    }
    /// Sets the order type of the specified node
    /// Returns the previous order type.
    ///
    /// # Errors
    /// Returns an [`Error`] when inputs do not match the inner tree state
    fn set_node_order_type(
        &mut self,
        node_path_str: &str,
        order_type: OrderType,
    ) -> Result<OrderType, Error> {
        let node_path = parse_path(node_path_str)?;
        let mut tree_guard = self.tree_guard();
        let mut node_ref = node_path.try_ref(&mut tree_guard)?;
        Ok(node_ref.set_order_type(order_type))
    }
    /// Updates the items for the specified node (and any children)
    ///
    /// # Errors
    /// Returns an [`Error`] when inputs do not match the inner tree state
    fn update_nodes(&mut self, node_path_str: &str) -> Result<(), Error> {
        let node_path = parse_path(node_path_str)?;
        // update node (recursively)
        let mut tree_guard = self.raw_tree.guard();
        Self::inner_update_node(&mut self.item_source, &node_path, &mut tree_guard)?;
        // TODO deleteme, no reason to repeat back (sanitized?) version of input param
        // Ok(serialize_path(node_path)?)
        Ok(())
    }
    fn inner_update_node<'a, 'b>(
        item_source: &mut T,
        path: impl Into<NodePathRefTyped<'a>>,
        tree_guard: &mut TreeGuard<'b, T, F>,
    ) -> Result<(), Error> {
        use q_filter_tree::iter::IterMutBreadcrumb;
        tree_guard
            .as_mut()
            .enumerate_mut_subtree_filters(path)?
            .with_all(|args, _path, mut node_ref| {
                let is_items = node_ref.child_nodes().is_none();
                if is_items {
                    // TODO add `NodeId` to the item, so user can diagnose
                    //   where specific queued item came from
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
    fn remove_node(
        &mut self,
        node_id_str: &str,
    ) -> Result<(q_filter_tree::Weight, NodeInfo<T, F>), Error> {
        let node_id = match parse_id(node_id_str)? {
            NodeIdTyped::Root(..) => Err(Error::Message("cannot remove root node".to_string())),
            NodeIdTyped::Child(child) => Ok(child),
        }?;
        let mut tree_guard = self.raw_tree.guard();
        let tree = tree_guard.as_mut();
        Ok(tree.remove_node(&node_id)??)
    }
    /// Returns the next [`Item`](`ItemSource::Item`)
    pub fn pop_next(&mut self) -> Option<SeqItem<T, F>> {
        let mut tree_guard = self.raw_tree.guard();
        let tree = tree_guard.as_mut();
        tree.pop_item()
            .map(|seq_item| seq_item.map(Cow::into_owned))
    }
    /// Returns an [`Iterator`] for the queue of the root node
    pub fn get_root_queue_items(&self) -> impl Iterator<Item = &SeqItem<T, F>> {
        let root_ref = self.tree_ref().root_node_shared();
        root_ref.queue_iter()
    }
    fn parse_path_or_root(
        &mut self,
        path: Option<&str>,
    ) -> Result<(NodePathTyped, TreeGuard<'_, T, F>), Error> {
        let mut tree_guard = self.tree_guard();
        let node_path = path
            .map(parse_path)
            .transpose()?
            .unwrap_or_else(|| tree_guard.as_mut().root_id().into());
        Ok((node_path, tree_guard))
    }
    fn set_node_prefill_count(
        &mut self,
        path: Option<&str>,
        min_count: usize,
    ) -> Result<(), Error> {
        let (node_path, mut tree_guard) = self.parse_path_or_root(path)?;
        let mut node_ref = node_path.try_ref(&mut tree_guard)?;
        node_ref.set_queue_prefill_len(min_count);
        Ok(())
    }
    // TODO add `item` to the specified Items node, at the index
    // fn queue_add_item(&mut self, path: Option<&str>, item: T, index: Option<usize>) -> Result<(), Error> {
    //     todo!()
    // }
    fn queue_remove_item(&mut self, path: Option<&str>, index: usize) -> Result<(), Error> {
        let (node_path, mut tree_guard) = self.parse_path_or_root(path)?;
        let mut node_ref = node_path.try_ref(&mut tree_guard)?;
        node_ref
            .try_queue_remove(index)
            .map(drop)
            .map_err(|queue_len| {
                Error::Message(format!(
                    "failed to remove from queue index {index}, max length {queue_len}"
                ))
            })
    }
}
/// [`Mismatch`] with associated label string
pub struct MismatchLabel<T>(Mismatch<T>, String);
impl<T> std::fmt::Display for MismatchLabel<T>
where
    T: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self(mismatch, label) = self;
        write!(f, "{label} {mismatch}")
    }
}
impl<T, F> Sequencer<T, Option<F>>
where
    T: ItemSource<Option<F>>,
    F: Clone,
{
    /// Calculates the required type `V` for the given path, with optional `requested_type`
    ///
    /// # Errors
    /// Returns [`Mismatch`] if the specified path type is incompatible with its ancestors, or the requested type (if any)
    pub fn calculate_required_type<V>(
        &self,
        path: &str,
        requested_type: Option<V>,
    ) -> Result<Result<Option<V>, MismatchLabel<V>>, Error>
    where
        V: for<'a> From<&'a F> + std::fmt::Debug + Eq,
    {
        let mut existing_path_type: Result<Option<(NodePathTyped, V)>, MismatchLabel<V>> = Ok(None);
        let mut accumulator = |path: &NodePathTyped, filter: &Option<F>| {
            let new_type = filter.as_ref().map(V::from);
            // detect and **REPORT** bad state
            if let Ok(existing_opt) = &mut existing_path_type {
                //TODO simplify in the future using Option::unzip
                // [tracking issue for Option::unzip](https://github.com/rust-lang/rust/issues/87800)
                let (existing_path, existing_type) = if let Some((path, ty)) = existing_opt.take() {
                    (Some(path), Some(ty))
                } else {
                    (None, None)
                };
                existing_path_type = Mismatch::combine_verify(new_type, existing_type)
                    .map(|matched| matched.map(|ty| (path.clone(), ty)))
                    .map_err(|mismatch| {
                        let existing_path_str = existing_path
                            .map_or_else(String::default, |p| format!(" from path {p}"));
                        MismatchLabel(mismatch, format!("path {path}{existing_path_str}"))
                    });
            }
        };
        self.with_ancestor_filters(path, &mut accumulator)?;
        Ok(existing_path_type
            .map(|path_type| path_type.map(|(_, ty)| ty))
            .and_then(|existing_type| {
                Mismatch::combine_verify(existing_type, requested_type)
                    .map_err(|mismatch| MismatchLabel(mismatch, path.to_string()))
            }))
    }
}

fn serialize_id<T: Into<NodeIdTyped>>(id: T) -> Result<NodeIdStr, serde_json::Error> {
    serde_json::to_string(&id.into()).map(NodeIdStr)
}
// TODO deleteme, unused
// fn serialize_path<T: Into<NodePathTyped>>(path: T) -> Result<Path, serde_json::Error> {
//     serde_json::to_string(&path.into()).map(Path)
// }
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

impl<T: ItemSource<F>, F> Sequencer<T, F>
where
    F: serde::Serialize + Clone,
{
    /// Returns a serializable representation of the inner [`Tree`](`q_filter_tree::Tree`)
    pub fn tree_serializable(&self) -> impl serde::Serialize + '_ {
        self.tree_ref()
    }
}
impl<T: ItemSource<F>, F> std::fmt::Display for Sequencer<T, F>
where
    F: serde::Serialize + Clone,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let tree = serde_json::to_string_pretty(self.tree_ref())
            .unwrap_or_else(|err| format!("<<Sequencer error: {err}>>"));
        write!(f, "{tree}")
    }
}

/// Serialized [`q_filter_tree::id::NodeId`]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeIdStr(pub String);
impl std::fmt::Display for NodeIdStr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// TODO deleteme, unused
// /// Serialized [`q_filter_tree::NodePath`]
// #[derive(Clone, Debug, PartialEq, Eq)]
// pub struct Path(pub String);
// impl std::fmt::Display for Path {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         write!(f, "{}", self.0)
//     }
// }

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
        { impl None for }
        /// Invalid index for child items
        InvalidItemIndex(NodePathTyped, usize),
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Message(err) => write!(f, "{err}"),
            Self::Serde(err) => write!(f, "serde error: {err}"),
            Self::InvalidNodePath(err) => write!(f, "{err}"),
            Self::RemoveError(err) => write!(f, "remove error: {err}"),
            Self::InvalidItemIndex(path, index) => {
                write!(f, "invalid item index {index} for path {path}")
            }
        }
    }
}

/// [`ItemSource`] used for debugging
#[derive(Clone, Default)]
pub struct DebugItemSource;
impl<T> ItemSource<T> for DebugItemSource
where
    T: std::fmt::Debug,
{
    type Item = String;
    type Error = shared::Never;

    fn lookup(&self, args: &[T]) -> Result<Vec<Self::Item>, Self::Error> {
        let debug_label = format!("{args:?}");
        Ok((0..10)
            .map(|n| format!("item # {n} for {}", &debug_label))
            .collect())
    }
}
