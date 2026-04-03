// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use super::{Cell, NodeDetails, NodeKind, Row, TableView, error::ViewError, error::count};
use crate::{
    Child, Network,
    child_vec::{ChildVec, Weights},
    order::OrderNode,
    path::{Path, PathRef},
};
use std::{ops::ControlFlow, rc::Rc};

mod experiment_non_recursive;

impl<T, U> Network<T, U> {
    /// Creates a [`TableView`] with default parameters
    ///
    /// See [`Self::view_table`] for details
    #[allow(clippy::missing_panics_doc, reason = "report bug in Network view")]
    pub fn view_table_default(&self) -> TableView {
        self.view_table(TableParams::default())
            .expect("table_view with default params should succeed")
    }
    /// Creates a [`TableView`]
    ///
    /// NOTE: each resulting node is either {Path/Id, Kind} or # omitted child nodes
    ///
    /// # Errors
    /// Returns an error if the specified path is not found, or the view dimensions are too large
    pub fn view_table(&self, table_params: TableParams<'_>) -> Result<TableView, ViewError> {
        let mut rows = vec![];
        let mut path = Path::empty();

        let mut item_node = &self.trees.item;
        let mut order_node = self.trees.order.node().get_children();
        let mut parent_active = true;
        let mut child_start_index = None;
        if let Some((child, parent_path)) = table_params.base_path.split_last() {
            child_start_index = Some(child);
            for index in parent_path {
                path.push(index);
                let weights = item_node.weights();
                item_node = match item_node.children().get(index) {
                    Some(Child::Joint(joint)) => Ok(&joint.next),
                    Some(Child::Bucket(_)) | None => {
                        Err(crate::UnknownPath(table_params.base_path.to_owned()))
                    }
                }?;
                order_node = match order_node.get(index) {
                    Some(node) => Ok(node.get_children()),
                    None => Err(crate::order::UnknownOrderPath(
                        table_params.base_path.to_owned(),
                    )),
                }?;
                parent_active = parent_active && weights.is_some_and(|w| w[index] != 0);
            }
        }

        let total_width = if item_node.is_empty() {
            // TODO why does this need to be a special case?  maybe adjust empty definition?
            0
        } else {
            TableBuilder::default().find_child_nodes(
                item_node,
                order_node,
                State {
                    depth: 0,
                    position: 0,
                    parent_active,
                    dest_cells: &mut rows,
                    params: table_params,
                    path_buf: &mut path,
                },
                child_start_index,
            )?
        };

        if false {
            let _ = experiment_non_recursive::run(table_params, &self.trees, (&rows, total_width));
        }

        Ok(TableView::new(rows, total_width))
    }
}

#[derive(Debug)]
struct State<'a, 'b, 'c> {
    depth: usize,
    position: u32,
    parent_active: bool,
    dest_cells: &'a mut Vec<Row>,
    params: TableParams<'b>,
    path_buf: &'c mut Path,
}

fn u32_limit(len: Option<u32>) -> u32 {
    len.unwrap_or(u32::MAX)
}

#[derive(Default)]
struct TableBuilder {
    node_count: u32,
}

impl TableBuilder {
    fn find_child_nodes<T, U>(
        &mut self,
        item_nodes: &ChildVec<Child<T, U>>,
        order_nodes: &[Rc<OrderNode>],
        mut state: State<'_, '_, '_>,
        child_start_index: Option<usize>,
    ) -> Result<u32, ViewError> {
        assert_eq!(
            item_nodes.len(),
            order_nodes.len(),
            "lengths should match between child items and child order"
        );

        let Some(item_nodes_max_index) = item_nodes.len().checked_sub(1) else {
            return Ok(1);
        };

        let result = state
            .params
            .trim_to_len(&mut self.node_count, item_nodes, state.depth);
        match result {
            ControlFlow::Continue(()) => {}
            ControlFlow::Break(result) => return Ok(result),
        }

        {
            let dest_cells = &mut *state.dest_cells;

            assert!(dest_cells.len() >= state.depth);
            if dest_cells.len() == state.depth {
                // add row for this depth
                dest_cells.push(Row::default());
            }
            assert!(dest_cells.len() > state.depth);
        }

        let weights = item_nodes.weights();
        if let Some(weights) = &weights {
            assert_eq!(weights.get_max_index(), item_nodes_max_index);
        }

        let parent_position = state.position;
        {
            let dest_row = state
                .dest_cells
                .get_mut(state.depth)
                .expect("row pushed above");
            let assumed_start = dest_row
                .get_cells()
                .iter()
                .map(Cell::get_display_width)
                .sum();
            assert!(assumed_start <= state.position);
            match state.position.checked_sub(assumed_start) {
                Some(gap_width) if gap_width > 0 => {
                    dest_row.push(Cell {
                        display_width: gap_width,
                        position: assumed_start,
                        parent_position: assumed_start,
                        node: None,
                    });
                }
                _ => {}
            }
        }

        let item_nodes = item_nodes.children();

        let (skip, take) = if let Some(child_start_index) = child_start_index {
            // skip to start
            let skip = child_start_index;
            // only take `max_width`
            let take = state
                .params
                .max_width
                .and_then(|v| usize::try_from(v).ok().map(|x| x + 1));
            (skip, take)
        } else {
            (0, None)
        };
        let item_and_order = {
            item_nodes
                .iter()
                .enumerate()
                .zip(order_nodes)
                .skip(skip)
                .take(take.unwrap_or(usize::MAX))
        };

        // TODO - this currently performs depth-first traversal (keeping track of which depth to
        // modify)... so use the common depth-first function? does that need extending?
        //  --> SEE module [`experiment_non_recursive`]
        let mut state = state;
        for ((index, child), order) in item_and_order {
            if matches!(state.params.max_width, Some(max_width) if state.position >= max_width) {
                let dest_row = state
                    .dest_cells
                    .get_mut(state.depth)
                    .expect("row pushed by caller, above");
                dest_row.push(Cell {
                    display_width: 0,
                    position: state.position,
                    parent_position,
                    node: None,
                });
                break;
            }

            state.add_child_node(self, parent_position, weights, ((index, child), order))?;
        }
        let total_width = state.position - parent_position;
        Ok(total_width)
    }
}
impl State<'_, '_, '_> {
    fn add_child_node<'a, T, U>(
        &mut self,
        table_builder: &mut TableBuilder,
        parent_position: u32,
        weights: Option<Weights<'_>>,
        ((index, child), order): ((usize, &'a Child<T, U>), &'a Rc<OrderNode>),
    ) -> Result<(), ViewError>
    where
        T: 'a,
        U: 'a,
    {
        // START - push index
        self.path_buf.push(index);

        let State {
            depth,
            position,
            parent_active,
            dest_cells: _,
            params,
            path_buf: _,
        } = *self;

        let weight = match weights {
            Some(weights) if weights.is_unity() => None,
            Some(weights) => Some(weights[index]),
            // no weights available means "all zero" weights
            None => Some(0),
        };
        let (kind, recurse) = match child {
            Child::Bucket(bucket) => {
                let item_count = count("bucket items length", bucket.items.len())?;
                (NodeKind::Bucket { item_count }, None)
            }
            Child::Joint(joint) => {
                let child_count = count("joint children length", joint.next.len())?;
                let children = &joint.next;
                match params.max_depth {
                    Some(max_depth) if count("depth", depth)? >= max_depth => {
                        (NodeKind::JointAbbrev { child_count }, None)
                    }
                    _ => (
                        NodeKind::Joint { child_count },
                        Some((children, order.get_children())),
                    ),
                }
            }
        };
        let active = parent_active && (weight != Some(0));

        let display_width = if let Some((item_nodes, order_nodes)) = recurse {
            let state = State {
                depth: depth + 1,
                position,
                parent_active: active,
                dest_cells: self.dest_cells,
                params,
                path_buf: self.path_buf,
            };
            table_builder
                .find_child_nodes(item_nodes, order_nodes, state, None)?
                .max(1)
        } else {
            1
        };

        let dest_row = self.dest_cells.get_mut(depth).expect("row pushed above");
        let node_details = NodeDetails {
            path: self.path_buf.clone(),
            active,
            weight,
            kind,
            order_type: order.get_order_type(),
        };
        dest_row.push(Cell {
            display_width,
            position,
            parent_position,
            node: Some(node_details),
        });

        self.position += display_width;

        // END - pop index
        self.path_buf.pop();

        Ok(())
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[allow(clippy::module_name_repetitions, reason = "name for re-export")]
/// Owned version of [`TableParams`] for use in serializing view requests
pub struct TableParamsOwned {
    max_depth: Option<u32>,
    max_width: Option<u32>,
    max_node_count: Option<u32>,
    base_path: Path,
}
/// Parameters for constructing a table view
#[derive(Clone, Copy, Debug)]
#[allow(clippy::module_name_repetitions, reason = "name for re-export")]
#[must_use]
pub struct TableParams<'a> {
    max_depth: Option<u32>,
    max_width: Option<u32>,
    max_node_count: Option<u32>,
    base_path: PathRef<'a>,
}
impl TableParamsOwned {
    /// Returns a reference version of the owned fields
    pub fn as_ref(&self) -> TableParams<'_> {
        let Self {
            max_depth,
            max_width,
            max_node_count,
            ref base_path,
        } = *self;
        TableParams {
            max_depth,
            max_width,
            max_node_count,
            base_path: base_path.as_ref(),
        }
    }
    // Modify functions for non-`Copy` types only
    /// Replaces the owned base [`Path`]
    pub fn set_base_path(&mut self, base_path: Path) {
        self.base_path = base_path;
    }
}
impl<'a> TableParams<'a> {
    /// Sets the maximum depth
    pub fn set_max_depth(mut self, max_depth: u32) -> Self {
        self.max_depth.replace(max_depth);
        self
    }
    /// Sets the maximum display width
    pub fn set_max_width(mut self, max_width: u32) -> Self {
        self.max_width.replace(max_width);
        self
    }
    /// Sets the maximum node count
    pub fn set_max_node_count(mut self, max_node_count: u32) -> Self {
        self.max_node_count.replace(max_node_count);
        self
    }
    /// Sets base [`Path`]
    pub fn set_base_path(mut self, base_path: PathRef<'a>) -> Self {
        self.base_path = base_path;
        self
    }
    /// Returns an owned version of the fields (cloning [`PathRef`] if any is set)
    #[must_use]
    pub fn to_owned(self) -> TableParamsOwned {
        let Self {
            max_depth,
            max_width,
            max_node_count,
            base_path,
        } = self;
        TableParamsOwned {
            max_depth,
            max_width,
            max_node_count,
            base_path: base_path.to_owned(),
        }
    }

    /// Returns the maximum depth
    #[must_use]
    pub fn get_max_depth(self) -> Option<u32> {
        self.max_depth
    }
    /// Returns the maximum display width
    #[must_use]
    pub fn get_max_width(self) -> Option<u32> {
        self.max_width
    }
    /// Returns the maximum node count (if any is set)
    #[must_use]
    pub fn get_max_node_count(self) -> Option<u32> {
        self.max_node_count
    }
}

impl Default for TableParamsOwned {
    fn default() -> Self {
        Self {
            max_depth: None,
            max_width: None,
            max_node_count: None,
            base_path: Path::empty(),
        }
    }
}
impl Default for TableParams<'_> {
    fn default() -> Self {
        Self {
            max_depth: None,
            max_width: None,
            max_node_count: None,
            base_path: PathRef::empty(),
        }
    }
}

impl TableParams<'_> {
    fn trim_to_len<T, U>(
        &mut self,
        node_count: &mut u32,
        item_nodes: &ChildVec<Child<T, U>>,
        state_depth: usize,
    ) -> ControlFlow<u32> {
        let display_len = {
            let child_len = u32::try_from(item_nodes.len()).unwrap_or(u32::MAX);
            child_len.min(u32_limit(self.max_width))
        };
        *node_count += display_len;

        // NOTE:
        // excess = node_count - max_node_count
        //
        // trim_to_len = display_len - excess
        // trim_to_len = display_len - (node_count - max_node_count)
        // trim_to_len = display_len - node_count + max_node_count
        if let Some(max_node_count) = self.max_node_count
            && let Some(excess) = node_count.checked_sub(max_node_count)
            && excess != 0
        {
            let Some(trim_to_len) = display_len.checked_sub(excess) else {
                return ControlFlow::Break(0);
            };

            self.max_width = Some(trim_to_len);
            self.max_depth = if trim_to_len == 0 {
                Some(0)
            } else {
                Some(u32_limit(state_depth.try_into().ok()))
            };
        }

        ControlFlow::Continue(())
    }
}
