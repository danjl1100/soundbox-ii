// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use super::{error::count, error::ViewError, Cell, NodeDetails, NodeKind, Row, TableView};
use crate::{
    child_vec::{ChildVec, Weights},
    order::OrderNode,
    path::{Path, PathRef},
    Child, Network,
};
use std::rc::Rc;

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
                parent_active = parent_active && weights.map_or(false, |w| w[index] != 0);
            }
        }

        let total_width = if item_node.is_empty() {
            // TODO why does this need to be a special case?  maybe adjust empty definition?
            0
        } else {
            table_params.find_child_nodes(
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

        Ok(TableView::new(rows, total_width))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct State {
    depth: usize,
    position: u32,
    parent_active: bool,
}
impl TableParams<'_> {
    fn find_child_nodes<T, U>(
        self,
        item_nodes: &ChildVec<Child<T, U>>,
        order_nodes: &[Rc<OrderNode>],
        dest_cells: &mut Vec<Row>,
        path_buf: &mut Path,
        state: State,
        child_start_index: Option<usize>,
    ) -> Result<u32, ViewError> {
        let Some(item_nodes_max_index) = item_nodes.len().checked_sub(1) else {
            return Ok(1);
        };

        assert!(dest_cells.len() >= state.depth);
        if dest_cells.len() == state.depth {
            // add row for this depth
            dest_cells.push(Row(vec![]));
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
            let take = self.max_width.and_then(|v| usize::try_from(v).ok());
            (skip, take)
        } else {
            (0, None)
        };
        let item_and_order = {
            assert_eq!(item_nodes.len(), order_nodes.len());
            item_nodes
                .iter()
                .enumerate()
                .zip(order_nodes)
                .skip(skip)
                .take(take.unwrap_or(usize::MAX))
        };

        // TODO - this currently performs depth-first traversal (keeping track of which depth to
        // modify)... so use the common depth-first function? does that need extending?
        let mut state = state;
        for ((index, child), order) in item_and_order {
            if matches!(self.max_width, Some(max_width) if state.position >= max_width) {
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
    fn add_child_node<'a, T, U>(
        self,
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
                match self.max_depth {
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
        let active = parent_active && weight.map_or(true, |w| w != 0);

        let display_width = if let Some((item_nodes, order_nodes)) = recurse {
            let state = State {
                depth: depth + 1,
                position,
                parent_active: active,
            };
            self.find_child_nodes(item_nodes, order_nodes, dest_cells, path_buf, state, None)?
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
    base_path: Path,
    // TODO add a max node count... to autodetect the depth based on how many total cells are seen
}
/// Parameters for constructing a table view
#[derive(Clone, Copy, Debug)]
#[allow(clippy::module_name_repetitions)]
#[must_use]
pub struct TableParams<'a> {
    max_depth: Option<u32>,
    max_width: Option<u32>,
    base_path: PathRef<'a>,
}
impl TableParamsOwned {
    /// Returns a reference version of the owned fields
    pub fn as_ref(&self) -> TableParams<'_> {
        let Self {
            max_depth,
            max_width,
            ref base_path,
        } = *self;
        TableParams {
            max_depth,
            max_width,
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
    /// Sets the maximum width
    pub fn set_max_width(mut self, max_width: u32) -> Self {
        self.max_width.replace(max_width);
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
            base_path,
        } = self;
        TableParamsOwned {
            max_depth,
            max_width,
            base_path: base_path.to_owned(),
        }
    }
}

impl Default for TableParamsOwned {
    fn default() -> Self {
        Self {
            max_depth: None,
            max_width: None,
            base_path: Path::empty(),
        }
    }
}
impl Default for TableParams<'_> {
    fn default() -> Self {
        Self {
            max_depth: None,
            max_width: None,
            base_path: PathRef::empty(),
        }
    }
}
