// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use super::{error::count, error::ViewError, Cell, NodeDetails, NodeKind, Row, TableView};
use crate::{
    child_vec::{ChildVec, Weights},
    order::OrderNode,
    path::{Path, PathRef},
    traversal::{ControlFlow, DepthFirstVisitor, TraversalElem},
    view::table_model::CellPartial,
    Child, Network, Trees,
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
    #[allow(clippy::missing_panics_doc, clippy::unwrap_used)] // TODO remove the test-only panics
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
                StateOld {
                    depth: 0,
                    position: 0,
                    parent_active,
                },
                child_start_index,
            )?
        };

        {
            let (rows_2, total_width_2) = table_params
                .build_rows(&self.trees)
                .expect("multiple error sources could get complicated");

            // let view = TableView::new(rows.clone(), total_width);
            // let view_2 = TableView::new(rows_2.clone(), total_width_2);

            if true {
                #[cfg(test)]
                {
                    let rows_json = serde_json::to_string_pretty(&rows).unwrap();
                    let rows_2_json = serde_json::to_string_pretty(&rows_2).unwrap();
                    let first_mismatch_line = rows_json
                        .lines()
                        .zip(rows_2_json.lines())
                        .enumerate()
                        .find(|(_idx, (s1, s2))| s1 != s2);
                    assert!(
                        rows_json == rows_2_json,
                        "Original: {rows_json}\nUpdated:{rows_2_json}\nFirst mismatched line {first_mismatch_line:?}"
                        // ... "\nOriginal {view}\nUpdated {view_2}"
                    );
                }
                assert_eq!(rows, rows_2, "rows should match old/new algorithms");
                assert_eq!(
                    total_width, total_width_2,
                    "total_width should match between old/new algorithms" // ... "\nOriginal {view}\nUpdated {view_2}"
                );
            }
        }

        Ok(TableView::new(rows, total_width))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct State {
    position: u32,
    parent_active: bool,
}
impl TableParams<'_> {
    fn build_rows<T, U>(self, trees: &Trees<T, U>) -> Result<(Vec<Row>, u32), ViewError> {
        let mut parent_active = true;
        let mut subtree = trees.subtree_scoped_at(self.base_path.to_owned(), |parent_elem| {
            parent_active = parent_active && parent_elem.node_weight != 0;
        })?;

        let mut visitor = TableBuilderVisitor {
            dest_cells: vec![],
            total_width: None,
            state_stack: vec![State {
                position: 0,
                parent_active,
            }],
            params: self,
            prev_visit_depth: None,
            prev_continuation_marker_needed: None,
        };

        subtree.try_visit_depth_first(&mut visitor)?;

        let TableBuilderVisitor {
            dest_cells,
            total_width,
            state_stack: _, // nothing to assert
            params: _,
            prev_visit_depth: _,
            prev_continuation_marker_needed: _,
        } = visitor;

        // TODO delete this old scrap
        // let total_width = state.position - parent_position;

        let rows: Result<Vec<_>, String> = dest_cells
            .into_iter()
            .map(|row| {
                let cells: Result<_, _> = row
                    .into_iter()
                    .map(|cp: CellPartial| Cell::try_from(cp))
                    .collect();
                Ok(Row::new(cells?))
            })
            .collect();
        let rows = rows.unwrap_or_else(|err| unreachable!("{err}"));
        let total_width = total_width.expect("final path should finalize once");
        let total_width = count("total width", total_width)?;
        Ok((rows, total_width))
    }
}

struct TableBuilderVisitor<'a> {
    dest_cells: Vec<Vec<CellPartial>>,
    total_width: Option<usize>,
    state_stack: Vec<State>,
    params: TableParams<'a>,
    prev_visit_depth: Option<usize>,
    prev_continuation_marker_needed: Option<usize>,
}
impl TableBuilderVisitor<'_> {
    fn get_depth(&self, path: PathRef<'_>) -> Option<usize> {
        // if self.base_path.is_empty() {
        //     // base_path is root --> depth = path_len - 1
        //     path.len().checked_sub(self.base_path.len() + 1)
        // } else {
        //     // base_path outside root --> depth = path_len - base_path_len
        //     path.len().checked_sub(self.base_path.len())
        // };
        let shortest_path_len_in_view = self.params.base_path.len().max(1);
        path.len().checked_sub(shortest_path_len_in_view)
    }
}
impl<T, U> DepthFirstVisitor<T, U, ViewError> for &mut TableBuilderVisitor<'_> {
    const FINALIZE: bool = true;
    #[allow(clippy::too_many_lines)] // TODO geez
    fn visit(
        &mut self,
        elem: TraversalElem<'_, crate::order::OrderNode, T, U>,
    ) -> Result<Result<(), ControlFlow>, ViewError> {
        let TraversalElem {
            node_path,
            parent_weights,
            node_weight,
            node_item,
            node_order,
        } = elem;

        let depth = self
            .get_depth(node_path)
            .expect("should visit paths at or below the base path");
        dbg!(("visit", &node_path, node_path.len(), depth));

        if let Some(prev_visit_depth) = self.prev_visit_depth {
            if depth < prev_visit_depth {
                println!(
                    "@{node_path} RESET parent_active to TRUE for depths {}..={prev_visit_depth}",
                    depth + 1
                );
                for prev_child_state in &mut self.state_stack[depth + 1..=prev_visit_depth] {
                    prev_child_state.parent_active = true;
                }
            }
        }

        if self.dest_cells.len() <= depth {
            self.dest_cells.resize(depth + 1, vec![]);
        }

        let parent_position = depth
            .checked_sub(1)
            .and_then(|parent_depth| self.state_stack.get(parent_depth))
            .map_or(0, |parent_state| parent_state.position);

        assert!(
            self.state_stack.len() > depth,
            "state stack not prepared for depth {depth} (current length {})",
            self.state_stack.len()
        );
        let Some(state) = self.state_stack.get_mut(depth) else {
            unreachable!(
                "state stack not prepared for depth {depth} (current length {})",
                self.state_stack.len(),
            )
        };

        state.position = state.position.max(parent_position);

        // println!("{:#?}", self.dest_cells);

        let dest_row = self
            .dest_cells
            .get_mut(depth)
            .expect("row should be pushed above");

        if let Some(prev_continuation_marker_needed) = self.prev_continuation_marker_needed.take() {
            if depth <= prev_continuation_marker_needed {
                dest_row.push(CellPartial {
                    display_width: Some(0),
                    position: state.position,
                    parent_position,
                    node: None,
                });
                return Ok(Err(ControlFlow::SkipAnyChildrenAndSiblings));
            }
        }
        if matches!(self.params.max_width, Some(max_width) if state.position >= max_width) {
            // // let dest_row = dest_cells
            // //     .get_mut(depth)
            // //     .expect("row pushed by caller, above");
            // dest_row.push(CellPartial {
            //     display_width: Some(0),
            //     position: state.position,
            //     parent_position,
            //     node: None,
            // });
            // // TODO figure out how to only emit zero-width continuation cell if another sibling exists
            // return Ok(Err(ControlFlow::SkipAnyChildrenAndSiblings));
            self.prev_continuation_marker_needed = Some(depth);
            return Ok(Ok(()));
        }

        {
            let assumed_start = dest_row
                .iter()
                .map(|cell| {
                    cell.display_width
                        .expect("display_width of sibling should be decided")
                })
                .sum();
            assert!(
                assumed_start <= state.position,
                "assumed_start {assumed_start}, state {state:?}"
            );
            println!("@{node_path} assumed_start {assumed_start}, state {state:?}");
            match state.position.checked_sub(assumed_start) {
                Some(gap_width) if gap_width > 0 => {
                    dest_row.push(CellPartial {
                        display_width: Some(gap_width),
                        position: assumed_start,
                        parent_position: assumed_start,
                        node: None,
                    });
                }
                _ => {}
            }
        }

        // let item_nodes = item_nodes.children();

        // let (skip, take) = if let Some(child_start_index) = child_start_index {
        //     // skip to start
        //     let skip = child_start_index;
        //     // only take `max_width`
        //     let take = self.max_width.and_then(|v| usize::try_from(v).ok());
        //     (skip, take)
        // } else {
        //     (0, None)
        // };
        // let item_and_order = {
        //     assert_eq!(item_nodes.len(), order_nodes.len());
        //     item_nodes
        //         .iter()
        //         .enumerate()
        //         .zip(order_nodes)
        //         .skip(skip)
        //         .take(take.unwrap_or(usize::MAX))
        // };

        // // TODO - this currently performs depth-first traversal (keeping track of which depth to
        // // modify)... so use the common depth-first function? does that need extending?
        // let mut state = state;
        // for ((index, child), order) in item_and_order {

        // if matches!(self.params.max_width, Some(max_width) if state.position >= max_width) {
        //     // let dest_row = dest_cells
        //     //     .get_mut(depth)
        //     //     .expect("row pushed by caller, above");
        //     dest_row.push(CellPartial {
        //         display_width: Some(0),
        //         position: state.position,
        //         parent_position,
        //         node: None,
        //     });
        //     // TODO figure out how to only emit zero-width continuation cell if another sibling exists
        //     return Ok(Err(ControlFlow::SkipAnyChildrenAndSiblings));
        // }

        let weight = if matches!(parent_weights, Some(weights) if weights.is_unity()) {
            None
        } else {
            Some(node_weight)
        };

        let child = node_item;
        let order = node_order;
        let (kind, recurse) = match child {
            Child::Bucket(bucket) => {
                let item_count = count("bucket items length", bucket.items.len())?;
                (NodeKind::Bucket { item_count }, Ok(None))
            }
            Child::Joint(joint) => {
                let child_count = count("joint children length", joint.next.len())?;
                match self.params.max_depth {
                    Some(max_depth) if count("depth", depth)? >= max_depth => (
                        NodeKind::JointAbbrev { child_count },
                        Err(ControlFlow::SkipAnyChildren),
                    ),
                    _ => (NodeKind::Joint { child_count }, Ok(Some(()))),
                }
            }
        };
        let active = state.parent_active && weight.map_or(true, |w| w != 0);

        let dest_row = self.dest_cells.get_mut(depth).expect("row pushed above");
        let node_details = NodeDetails {
            path: node_path.to_owned(),
            active,
            weight,
            kind,
            order_type: order.get_order_type(),
        };
        dest_row.push(CellPartial {
            // NOTE: child nodes have not rendered quite yet
            display_width: None, // marker for future update
            position: state.position,
            parent_position,
            node: Some(node_details),
        });

        let control_flow_result = match recurse {
            Ok(Some(())) => {
                let child_state = State {
                    position: state.position,
                    parent_active: active,
                };
                if self.state_stack.len() == depth + 1 {
                    self.state_stack.push(child_state);
                }
                Ok(())
            }
            Ok(None) => Ok(()),
            Err(break_control_flow) => Err(break_control_flow),
        };

        self.prev_visit_depth = Some(depth);

        Ok(control_flow_result)
    }
    fn finalize_after_children(
        &mut self,
        path: PathRef<'_>,
        child_sum: usize,
    ) -> Result<usize, ViewError> {
        dbg!(("finalize_after_children", path, child_sum));
        let display_width = if child_sum == 0 { 1 } else { child_sum };
        if let Some(depth) = self.get_depth(path) {
            let display_width = count("display width", display_width)?;
            if self.prev_continuation_marker_needed.is_none() {
                let state = self
                    .state_stack
                    .get_mut(depth)
                    .expect("visit should populate state_stack for finalize fn");
                state.position += display_width;
            }
            if self.prev_continuation_marker_needed.is_none() {
                let last_cell = self
                    .dest_cells
                    .get_mut(depth)
                    .and_then(|row| row.last_mut())
                    .expect("visit should push cell for finalize fn");
                println!(
                    "@{path}?={} setting last_cell.display_width (was {:?})",
                    last_cell
                        .node
                        .as_ref()
                        .map_or(&"" as &dyn std::fmt::Display, |cell| &cell.path),
                    last_cell.display_width
                );
                assert_eq!(
                    last_cell.display_width, None,
                    "should only set cell position once"
                );
                last_cell.display_width = Some(display_width);
            }
        } else {
            let prev = self.total_width.replace(child_sum);
            assert!(prev.is_none(), "final path should finalize only once");
        }
        Ok(display_width)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct StateOld {
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
        state: StateOld,
        child_start_index: Option<usize>,
    ) -> Result<u32, ViewError> {
        let Some(item_nodes_max_index) = item_nodes.len().checked_sub(1) else {
            return Ok(1);
        };

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
            let take = self
                .max_width
                .and_then(|v| usize::try_from(v).ok().map(|x| x + 1));
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
        StateOld {
            depth,
            position,
            parent_active,
        }: StateOld,
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
            let state = StateOld {
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
