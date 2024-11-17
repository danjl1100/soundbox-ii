// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use super::TableParams;
use crate::{
    path::PathRef,
    traversal::{ControlFlow, DepthFirstVisitor, TraversalElem},
    view::{
        error::{count, ViewError},
        table_model::CellPartial,
        Cell, NodeDetails, NodeKind, Row, TableView,
    },
    Child, Trees,
};

#[allow(clippy::missing_panics_doc, clippy::unwrap_used)] // TODO remove the test-only panics
pub(super) fn run<T, U>(
    table_params: TableParams,
    trees: &Trees<T, U>,
    (expected_rows, expected_total_width): (&[Row], u32),
) -> TableView {
    let (rows, total_width) = table_params
        .build_rows(trees)
        .expect("multiple error sources could get complicated");

    // let expected_view = TableView::new(rows.clone(), total_width);
    // let view = TableView::new(rows.clone(), total_width);

    #[cfg(test)]
    {
        let expected_rows_json = serde_json::to_string_pretty(&expected_rows).unwrap();
        let rows_2_json = serde_json::to_string_pretty(&rows).unwrap();
        let first_mismatch_line = expected_rows_json
            .lines()
            .zip(rows_2_json.lines())
            .enumerate()
            .find(|(_idx, (s1, s2))| s1 != s2);
        assert!(
            expected_rows_json == rows_2_json,
            "Original: {expected_rows_json}\nUpdated:{rows_2_json}\nFirst mismatched line {first_mismatch_line:?}"
            // ... "\nOriginal {expected_view}\nUpdated {view}"
        );
    }
    assert_eq!(expected_rows, rows, "rows should match old/new algorithms");
    assert_eq!(
        expected_total_width, total_width,
        "total_width should match between old/new algorithms" // ... "\nOriginal {expected_view}\nUpdated {view}"
    );

    TableView::new(rows, total_width)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct State {
    position: u32,
    parent_active: bool,
}
impl TableParams<'_> {
    pub(super) fn build_rows<T, U>(
        self,
        trees: &Trees<T, U>,
    ) -> Result<(Vec<Row>, u32), ViewError> {
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
    // fn is_finalize_required() -> bool {
    //     true
    // }
}
