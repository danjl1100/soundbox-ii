// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use super::{error::count, error::ViewError, Cell, NodeDetails, NodeKind, Row, TableView};
use crate::{
    child_vec::{ChildVec, Weights},
    order::OrderNode,
    path::{Path, PathRef},
    Child, Network,
};
use std::rc::Rc;

mod experiment_non_recursive;

impl<T, U> Network<T, U> {
    /// Creates a [`TableView`] with default parameters
    ///
    /// See [`Self::view_table`] for details
    #[allow(clippy::missing_panics_doc)]
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
                table_params,
                item_node,
                order_node,
                &mut rows,
                &mut path,
                State {
                    depth: 0,
                    position: 0,
                    parent_active,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct State {
    depth: usize,
    position: u32,
    parent_active: bool,
}

fn u32_limit(len: Option<u32>) -> u32 {
    len.unwrap_or(u32::MAX)
}

#[derive(Default)]
struct TableBuilder {
    node_count: u32,
}

impl TableBuilder {
    #[allow(clippy::too_many_lines)] // TODO yikes..
    #[allow(clippy::too_many_arguments)] // TODO double yikes, arg..
    fn find_child_nodes<T, U>(
        &mut self,
        mut params: TableParams<'_>,
        item_nodes: &ChildVec<Child<T, U>>,
        order_nodes: &[Rc<OrderNode>],
        dest_cells: &mut Vec<Row>,
        path_buf: &mut Path,
        state: State,
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

        let display_len = {
            let child_len = u32::try_from(item_nodes.len()).unwrap_or(u32::MAX);
            child_len.min(u32_limit(params.max_width))
        };
        self.node_count += display_len;
        let trim_to_len = params.max_node_count.and_then(|max_node_count| {
            if let Some(excess) = self.node_count.checked_sub(max_node_count) {
                if excess == 0 {
                    // TODO debug
                    // println!(
                    //     "excess is 0, node_count {node_count}, max_node_count {max_node_count}",
                    //     node_count = self.node_count
                    // );
                    None
                } else {
                    // NOTE:
                    // excess = node_count - max_node_count
                    //
                    // trim_to_len = display_len - excess
                    // trim_to_len = display_len - (node_count - max_node_count)
                    // trim_to_len = display_len - node_count + max_node_count
                    Some(display_len.checked_sub(excess))
                }
            } else {
                // TODO debug
                // println!(
                //     "excess is negative, node_count {node_count}, max_node_count {max_node_count}",
                //     node_count = self.node_count
                // );
                None
            }
        });

        match trim_to_len {
            Some(None) => return Ok(0),
            Some(Some(trim_to_len)) => {
                // dbg!(("before", &params));
                params.max_width = Some(trim_to_len);
                params.max_depth = if trim_to_len == 0 {
                    Some(0)
                } else {
                    Some(u32_limit(state.depth.try_into().ok()))
                };
                // dbg!(("after", trim_to_len, &params));
            }
            None => {}
        }

        assert!(dest_cells.len() >= state.depth);
        if dest_cells.len() == state.depth {
            // add row for this depth
            dest_cells.push(Row::default());
        }
        assert!(dest_cells.len() > state.depth);

        let weights = item_nodes.weights();
        if let Some(weights) = &weights {
            assert_eq!(weights.get_max_index(), item_nodes_max_index);
        }

        let parent_position = state.position;
        {
            let dest_row = dest_cells.get_mut(state.depth).expect("row pushed above");
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
            let take = params
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
            if matches!(params.max_width, Some(max_width) if state.position >= max_width) {
                let dest_row = dest_cells
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

            // START - push index
            path_buf.push(index);

            let display_width = self.add_child_node(
                params,
                dest_cells,
                path_buf,
                state,
                parent_position,
                weights,
                ((index, child), order),
            )?;

            state.position += display_width;

            // END - pop index
            path_buf.pop();
        }
        let total_width = state.position - parent_position;
        Ok(total_width)
    }
    #[allow(clippy::too_many_arguments)] // TODO double yikes, arg..
    fn add_child_node<'a, T, U>(
        &mut self,
        params: TableParams<'_>,
        dest_cells: &mut Vec<Row>,
        path_buf: &mut Path,
        State {
            depth,
            position,
            parent_active,
        }: State,
        parent_position: u32,
        weights: Option<Weights<'_>>,
        ((index, child), order): ((usize, &'a Child<T, U>), &'a Rc<OrderNode>),
    ) -> Result<u32, ViewError>
    where
        T: 'a,
        U: 'a,
    {
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
            };
            self.find_child_nodes(
                params,
                item_nodes,
                order_nodes,
                dest_cells,
                path_buf,
                state,
                None,
            )?
            .max(1)
        } else {
            1
        };

        let dest_row = dest_cells.get_mut(depth).expect("row pushed above");
        let node_details = NodeDetails {
            path: path_buf.clone(),
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

        Ok(display_width)
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[allow(clippy::module_name_repetitions)]
/// Owned version of [`TableParams`] for use in serializing view requests
pub struct TableParamsOwned {
    max_depth: Option<u32>,
    max_width: Option<u32>,
    max_node_count: Option<u32>,
    base_path: Path,
}
/// Parameters for constructing a table view
#[derive(Clone, Copy, Debug)]
#[allow(clippy::module_name_repetitions)]
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
