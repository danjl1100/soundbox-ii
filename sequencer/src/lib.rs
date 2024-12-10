// soundbox-ii/sequencer music playback controller *don't keep your sounds boxed up*
// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
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

// TODO - restore after finalizing flake
// // teach me
// #![deny(clippy::pedantic)]
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
    id::{ty, NodeId, NodeIdTyped, NodePathRefTyped, NodePathTyped, SequenceSource},
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

pub mod cli;

pub mod persistence;

// conversions, for ergonomic use with `ItemSource`
type Item<T, F> = <T as ItemSource<F>>::Item;
type SeqItem<T, F> = q_filter_tree::SequenceAndItem<Item<T, F>>;
// type GuardedTree<T, F> = q_filter_tree::GuardedTree<Item<T, F>, F>;
// type TreeGuard<'a, T, F> = q_filter_tree::TreeGuard<'a, Item<T, F>, F>;
type NodeInfo<T, F> = q_filter_tree::NodeInfo<Item<T, F>, F>;

/// Sequencer for tracks from a user-specified source
#[derive(Default)]
pub struct Sequencer<T: ItemSource<F>, F> {
    inner: SequencerTree<Item<T, F>, F>,
    item_source: T,
}
/// Tree of filters for selecting tracks, using [`q_filter_tree`] back-end
pub struct SequencerTree<T, F> {
    tree: q_filter_tree::GuardedTree<T, F>,
}
struct SequencerTreeGuard<'a, T: Clone, F> {
    guard: q_filter_tree::TreeGuard<'a, T, F>,
}
impl<T: ItemSource<F>, F> Sequencer<T, F> {
    /// Creates a new, empty Sequencer
    pub fn new(item_source: T, root_filter: F) -> Self {
        Self {
            inner: SequencerTree::new(root_filter),
            item_source,
        }
    }
    /// Creates a new Sequencer from the specified tree
    ///
    /// (e.g. from [`persistence::SequencerConfigFile::read_from_file`])
    pub fn new_from_tree(item_source: T, inner: SequencerTree<Item<T, F>, F>) -> Self {
        Self { item_source, inner }
    }

    /// Replaces the inner tree
    pub fn replace_tree(
        &mut self,
        new_tree: SequencerTree<Item<T, F>, F>,
    ) -> SequencerTree<Item<T, F>, F> {
        std::mem::replace(&mut self.inner, new_tree)
    }
}
impl<T: Clone, F> SequencerTree<T, F> {
    /// Creates a new, empty [`SequencerTree`]
    pub fn new(root_filter: F) -> Self {
        Self {
            tree: q_filter_tree::Tree::new_with_filter(root_filter).into(),
        }
    }
}
impl<T, F> SequencerTree<T, F>
where
    T: Clone,
    F: Clone,
{
    fn guard(&mut self) -> SequencerTreeGuard<'_, T, F> {
        let guard = self.tree.guard();
        SequencerTreeGuard { guard }
    }
}
// TODO move this impl to submodule, to clarify not using NodeIsStr (and clarify pub(crate))
impl<T, F> SequencerTreeGuard<'_, T, F>
where
    T: Clone,
    F: Clone,
{
    fn inner_add_node(
        &mut self,
        parent_path: NodePathRefTyped<'_>,
        filter: F,
    ) -> Result<NodeId<ty::Child>, Error> {
        let tree_guard = &mut self.guard;
        let mut parent_ref = parent_path.try_ref(tree_guard)?;
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
    fn add_node(
        &mut self,
        parent_path: NodePathRefTyped<'_>,
        filter: F,
    ) -> Result<NodeId<ty::Child>, Error> {
        let new_node_id = self.inner_add_node(parent_path, filter)?;
        Ok(new_node_id)
    }
    /// Adds a terminal `Node` to the specified path.
    /// Returns the path of the created node.
    ///
    /// # Errors
    /// Returns an [`Error`] when inputs do not match the inner tree state
    fn add_terminal_node(
        &mut self,
        parent_path: NodePathRefTyped<'_>,
        filter: F,
    ) -> Result<NodeId<ty::Child>, Error> {
        let new_node_id = self.inner_add_node(parent_path, filter)?;
        let tree_guard = &mut self.guard;
        let mut node_ref = new_node_id.try_ref(tree_guard)?;
        node_ref.overwrite_child_items_uniform(std::iter::empty());
        Ok(new_node_id)
    }
    /// Sets the filter of the specified node
    /// Returns the previous filter value.
    ///
    /// # Errors
    /// Returns an [`Error`] when inputs do not match the inner tree state
    fn set_node_filter(&mut self, node_path: NodePathRefTyped<'_>, filter: F) -> Result<F, Error> {
        let tree_guard = &mut self.guard;
        let mut node_ref = node_path.try_ref(tree_guard)?;
        let old_filter = std::mem::replace(&mut node_ref.filter, filter);
        Ok(old_filter)
    }
    /// Sets the weight of the specified item in the node
    /// Returns the previous weight value.
    ///
    /// # Errors
    /// Returns an [`Error`] when inputs do not match the inner tree state
    fn set_node_item_weight(
        &mut self,
        node_path: NodePathRefTyped<'_>,
        item_index: usize,
        weight: Weight,
    ) -> Result<Weight, Error> {
        let tree_guard = &mut self.guard;
        let mut node_ref = node_path.try_ref(tree_guard)?;
        let child_items = node_ref.child_items().ok_or_else(|| {
            Error::Message(format!(
                "cannot set items for node at path {node_path}, type is chain"
            ))
        })?;
        let mut child_items = child_items.ref_mut();
        match child_items.set_weight(item_index, weight) {
            Ok(old_weight) => Ok(old_weight),
            Err(invalid_index) => Err(Error::InvalidItemIndex(
                node_path.clone_owned(),
                invalid_index,
            )),
        }
    }
    /// Sets the weight of the specified node
    /// Returns the previous weight value.
    ///
    /// # Errors
    /// Returns an [`Error`] when inputs do not match the inner tree state
    fn set_node_weight(
        &mut self,
        node_path: NodePathRefTyped<'_>,
        weight: Weight,
    ) -> Result<Weight, Error> {
        let node_path = match node_path {
            NodePathRefTyped::Root(path) => Err(InvalidNodePath::from(*path)),
            NodePathRefTyped::Child(path) => Ok(path),
        }?;
        let tree_guard = &mut self.guard;
        let mut node_ref = node_path.try_ref(tree_guard)?;
        Ok(node_ref.set_weight(weight))
    }
    /// Sets the order type of the specified node
    /// Returns the previous order type.
    ///
    /// # Errors
    /// Returns an [`Error`] when inputs do not match the inner tree state
    fn set_node_order_type(
        &mut self,
        node_path: NodePathRefTyped<'_>,
        order_type: OrderType,
    ) -> Result<OrderType, Error> {
        let tree_guard = &mut self.guard;
        let mut node_ref = node_path.try_ref(tree_guard)?;
        Ok(node_ref.set_order_type(order_type))
    }
    /// Removes a `Node` at the specified id (path`#`sequence)
    ///
    /// # Errors
    /// Returns an [`Error`] when inputs do not match the inner tree state
    fn remove_node(
        &mut self,
        node_id: &NodeId<ty::Child>,
    ) -> Result<(q_filter_tree::Weight, q_filter_tree::NodeInfo<T, F>), Error> {
        let tree = self.guard.as_mut();
        Ok(tree.remove_node(node_id)??)
    }
    /// Returns the next [`Item`](`ItemSource::Item`)
    fn pop_next(&mut self) -> Option<q_filter_tree::SequenceAndItem<T>> {
        let tree = self.guard.as_mut();
        tree.pop_item()
            .map(|seq_item| seq_item.map(Cow::into_owned))
    }
    fn set_node_prefill_count(
        &mut self,
        node_path: NodePathRefTyped<'_>,
        min_count: usize,
    ) -> Result<(), Error> {
        let tree_guard = &mut self.guard;
        let mut node_ref = node_path.try_ref(tree_guard)?;
        node_ref.set_queue_prefill_len(min_count);
        Ok(())
    }
    // TODO add `item` to the specified Items node, at the index
    // fn queue_add_item(&mut self, path: Option<&str>, item: T, index: Option<usize>) -> Result<(), Error> {
    //     todo!()
    // }
    fn queue_remove_item(
        &mut self,
        node_path: NodePathRefTyped<'_>,
        index: usize,
    ) -> Result<(), Error> {
        let tree_guard = &mut self.guard;
        let mut node_ref = node_path.try_ref(tree_guard)?;
        node_ref
            .try_queue_remove(index)
            .map(drop)
            .map_err(|queue_len| {
                Error::Message(format!(
                    "failed to remove from queue index {index}, max length {queue_len}"
                ))
            })
    }
    fn move_node(
        &mut self,
        src_id: &NodeId<ty::Child>,
        dest_id: NodeIdTyped,
    ) -> Result<NodeId<ty::Child>, Error> {
        let tree_guard = &mut self.guard;

        // verify destination node is non-terminal
        dest_id
            .try_ref(tree_guard)?
            .child_nodes()
            .map(|_| ())
            .ok_or_else(|| format!("Node {dest_id} does not have child_nodes"))?;
        // detect path changing (later sibling of the src) and adjust accordingly
        let dest_id_reresolved = resolved_post_remove_id(src_id, dest_id)
            .ok_or_else(|| "cannot move node to itself".to_string())?;

        // remove from source location
        let (_removed_weight, removed_info) = tree_guard.as_mut().remove_node(src_id)??;

        // insert at destination location
        let mut dest_ref = dest_id_reresolved.try_ref(tree_guard)?;
        let mut child_nodes = dest_ref
            .child_nodes()
            .expect("pre-validated Node now suddently does not have child_nodes");
        let new_node_id = child_nodes.add_child_default_from(removed_info);

        Ok(new_node_id)
    }
}
impl<T: ItemSource<F>, F> Sequencer<T, F>
where
    F: Clone,
{
    /// Adds a `Node` to the specified path.
    /// Returns the path of the created node.
    ///
    /// # Errors
    /// Returns an [`Error`] when inputs do not match the inner tree state
    fn add_node(&mut self, parent_path_str: &str, filter: F) -> Result<NodeIdStr, Error> {
        let parent_path = parse_path(parent_path_str)?;
        let parent_path = (&parent_path).into();
        let new_node_id = self.inner.guard().add_node(parent_path, filter)?;
        Ok(serialize_id(NodeIdTyped::from(new_node_id))?)
    }
    /// Adds a terminal `Node` to the specified path.
    /// Returns the path of the created node.
    ///
    /// # Errors
    /// Returns an [`Error`] when inputs do not match the inner tree state
    fn add_terminal_node(&mut self, parent_path_str: &str, filter: F) -> Result<NodeIdStr, Error> {
        let parent_path = parse_path(parent_path_str)?;
        let parent_path = (&parent_path).into();
        let mut tree_guard = self.inner.guard();
        let new_node_id = tree_guard.add_terminal_node(parent_path, filter)?;
        Self::inner_update_node(&self.item_source, &new_node_id, &mut tree_guard)?;
        Ok(serialize_id(new_node_id)?)
    }
    /// Sets the filter of the specified node
    /// Returns the previous filter value.
    ///
    /// # Errors
    /// Returns an [`Error`] when inputs do not match the inner tree state
    fn set_node_filter(&mut self, node_path_str: &str, filter: F) -> Result<F, Error> {
        let mut tree_guard = self.inner.guard();
        let node_path = parse_path(node_path_str)?;
        let old_filter = tree_guard.set_node_filter((&node_path).into(), filter)?;
        Self::inner_update_node(&self.item_source, &node_path, &mut tree_guard)?;
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
        self.inner
            .guard()
            .set_node_item_weight((&node_path).into(), item_index, weight)
    }
    /// Sets the weight of the specified node
    /// Returns the previous weight value.
    ///
    /// # Errors
    /// Returns an [`Error`] when inputs do not match the inner tree state
    fn set_node_weight(&mut self, node_path_str: &str, weight: Weight) -> Result<Weight, Error> {
        let node_path = parse_path(node_path_str)?;
        self.inner
            .guard()
            .set_node_weight((&node_path).into(), weight)
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
        self.inner
            .guard()
            .set_node_order_type((&node_path).into(), order_type)
    }
    /// Updates the items for the specified node (and any children)
    ///
    /// # Errors
    /// Returns an [`Error`] when inputs do not match the inner tree state
    fn update_nodes(&mut self, node_path_str: &str) -> Result<(), Error> {
        let node_path = parse_path(node_path_str)?;
        // update node (recursively)
        let mut tree_guard = self.inner.guard();
        Self::inner_update_node(&self.item_source, &node_path, &mut tree_guard)?;
        // TODO deleteme, no reason to repeat back (sanitized?) version of input param
        // Ok(serialize_path(node_path)?)
        Ok(())
    }
    fn inner_update_node<'a, 'b>(
        item_source: &T,
        path: impl Into<NodePathRefTyped<'a>>,
        tree_guard: &mut SequencerTreeGuard<'b, Item<T, F>, F>,
    ) -> Result<(), Error> {
        use q_filter_tree::iter::IterMutBreadcrumb;
        tree_guard
            .guard
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
        let node_id = parse_id_child(node_id_str, "remove")?;
        self.inner.guard().remove_node(&node_id)
    }
    /// Returns the next [`Item`](`ItemSource::Item`)
    pub fn pop_next(&mut self) -> Option<SeqItem<T, F>> {
        self.inner.guard().pop_next()
    }
    fn set_node_prefill_count(
        &mut self,
        path: Option<&str>,
        min_count: usize,
    ) -> Result<(), Error> {
        let node_path = self.parse_path_or_root(path)?;
        self.inner
            .guard()
            .set_node_prefill_count((&node_path).into(), min_count)
    }
    fn queue_remove_item(&mut self, node_path: Option<&str>, index: usize) -> Result<(), Error> {
        let node_path = self.parse_path_or_root(node_path)?;
        self.inner
            .guard()
            .queue_remove_item((&node_path).into(), index)
    }
    fn move_node(&mut self, src_id_str: &str, dest_id_str: &str) -> Result<NodeIdStr, Error> {
        let src_id = parse_id_child(src_id_str, "move")?;
        let dest_id = parse_id(dest_id_str)?;
        let new_node_id = self.inner.guard().move_node(&src_id, dest_id)?;
        Ok(serialize_id(new_node_id)?)
    }
    fn parse_path_or_root(&self, path: Option<&str>) -> Result<NodePathTyped, Error> {
        let node_path = path
            .map(parse_path)
            .transpose()?
            .unwrap_or_else(|| self.inner.tree.root_id().into());
        Ok(node_path)
    }
    /// Returns an [`Iterator`] for the queue of the root node
    pub fn get_root_queue_items(&self) -> impl Iterator<Item = &SeqItem<T, F>> {
        let root_ref = self.inner.tree.root_node_shared();
        root_ref.queue_iter()
    }
}
/// Returns the input `NodeId`, modified to reflect removal of the specified `NodeId`
/// Note: Returns `None` if the two provided `NodeId`s have identical paths
///
/// Note: Assumes (via `q_filter_tree` rules) that the removed node has no child nodes
///
/// This detects path changes for moving a node to a path underneath a later-sibling
fn resolved_post_remove_id(
    removed_id: &NodeId<ty::Child>,
    input: NodeIdTyped,
) -> Option<NodeIdTyped> {
    let input_seq = input.sequence_keeper();
    let resolved_path: Option<NodePathTyped> = match input.into() {
        NodePathTyped::Root(root) => Some(root.into()),
        NodePathTyped::Child(input) => {
            let (removed_end, removed_parent) = removed_id.elems_split_last();
            let (input_end, input_parent) = input.elems_split_last();
            if input_parent == removed_parent {
                match input_end.cmp(&removed_end) {
                    std::cmp::Ordering::Less => Some(input.into()),
                    std::cmp::Ordering::Equal => None,
                    std::cmp::Ordering::Greater => {
                        let result_end = input_end - 1;
                        let result_path = (input_parent.iter().copied().collect::<NodePathTyped>())
                            .append(result_end);
                        Some(result_path.into())
                    }
                }
            } else {
                Some(input.into())
            }
        }
    };
    resolved_path.map(|path| path.with_sequence(&input_seq))
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
                let (existing_path, existing_type) = existing_opt.take().unzip();
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

impl<T: Clone, F: Default> Default for SequencerTree<T, F> {
    fn default() -> Self {
        SequencerTree {
            tree: q_filter_tree::Tree::default().into(),
        }
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
fn parse_id_child(input_str: &str, operation: &str) -> Result<NodeId<ty::Child>, String> {
    let node_id = parse_id(input_str)?;
    let sequence = node_id.sequence_keeper();
    match node_id.into() {
        NodePathTyped::Root(..) => Err(format!("cannot {operation} root node")),
        NodePathTyped::Child(child) => Ok(child.with_sequence(&sequence)),
    }
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
        &*self.inner.tree
    }
    /// Returns the sequencer tree
    ///
    /// e.g. for use with [`persistence::SequencerConfig::update_to_string`] or
    /// [`persistence::SequencerConfigFile::update_to_file`]
    pub fn sequencer_tree(&self) -> &SequencerTree<T::Item, F> {
        &self.inner
    }
}
impl<T: ItemSource<F>, F> std::fmt::Display for Sequencer<T, F>
where
    F: serde::Serialize + Clone,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let tree = serde_json::to_string_pretty(&*self.inner.tree)
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
