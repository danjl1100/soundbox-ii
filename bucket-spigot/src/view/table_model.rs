// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use crate::{
    order::OrderType,
    path::{Path, PathRef},
};

/// Tabular view of a [`Network`](`crate::Network`)
#[derive(Clone, PartialEq, Eq, serde::Serialize)]
#[must_use]
pub struct TableView {
    rows: Vec<Row>,
    total_width: u32,
}
/// Sequence of [`Cell`]s at the same depth
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct Row(Vec<Cell>);

/// There are three kinds of `Cell`:
///
///    1. Node, when: `display_width > 0`, `node = Some(_)`
///    2. Spacer, when: `display_width > 0`, `node = None`
///    3. Horizontal continuation marker (column width-wise), when: `display_width = 0`, `node = None`
///
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Cell {
    pub(super) display_width: u32,
    pub(super) position: u32,
    pub(super) parent_position: u32,
    pub(super) node: Option<NodeDetails>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub(super) struct CellPartial {
    pub(super) display_width: Option<u32>,
    pub(super) position: u32,
    pub(super) parent_position: u32,
    pub(super) node: Option<NodeDetails>,
}
impl TryFrom<CellPartial> for Cell {
    type Error = String;
    fn try_from(value: CellPartial) -> Result<Self, Self::Error> {
        let CellPartial {
            display_width,
            position,
            parent_position,
            node,
        } = value;
        let display_width = display_width.ok_or_else(|| {
            let node = node
                .as_ref()
                .map_or(&"spacer" as &dyn std::fmt::Display, |node| node);
            format!("display_width is None for {node}")
        })?;
        Ok(Self {
            display_width,
            position,
            parent_position,
            node,
        })
    }
}

/// Details for a node
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct NodeDetails {
    pub(super) path: Path,
    /// True if the node is reachable from the spigot root
    pub(super) active: bool,
    /// Weight of the node relative to siblings (or `None` if all equal)
    pub(super) weight: Option<u32>,
    pub(super) kind: NodeKind,
    pub(super) order_type: OrderType,
    // NOTE: exclude Filters list as it is relatively unbounded
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub(super) enum NodeKind {
    /// Bucket node
    Bucket { item_count: u32 },
    /// Joint node
    Joint { child_count: u32 },
    /// Vertical continuation marker (row depth-wise) - joint node with
    /// children are hidden by `max_depth`
    JointAbbrev { child_count: u32 },
}

impl TableView {
    /// Returns the [`Row`]s in the table
    #[must_use]
    pub fn get_rows(&self) -> &[Row] {
        &self.rows
    }
    /// Returns the sum of display widths for the largest row
    #[must_use]
    pub fn get_max_row_width(&self) -> u32 {
        self.total_width
    }
}
impl Row {
    pub(super) fn new(elems: Vec<Cell>) -> Self {
        Self(elems)
    }
    /// Returns the [`Cell`]s in the row
    #[must_use]
    pub fn get_cells(&self) -> &[Cell] {
        &self.0
    }
    pub(super) fn push(&mut self, cell: Cell) {
        self.0.push(cell);
    }
    #[allow(unused)] // TODO remove if removing `crate::view::table::experiment_non_recursive`
    pub(super) fn last_mut(&mut self) -> Option<&mut Cell> {
        self.0.last_mut()
    }
}
impl Cell {
    /// Returns the width for displaying the [`Cell`]
    #[must_use]
    pub fn get_display_width(&self) -> u32 {
        self.display_width
    }
    /// Returns the node at the [`Cell`] (if any, otherwise it is only a spacer)
    #[must_use]
    pub fn get_node(&self) -> Option<&NodeDetails> {
        self.node.as_ref()
    }
    /// Returns the display width sum for all prior cells in the row
    #[must_use]
    pub fn get_position(&self) -> u32 {
        self.position
    }
    /// Returns the position for the parent cell in the prior row (or 0 for the first row)
    ///
    /// See also [`Cell::get_position`]
    #[must_use]
    pub fn get_parent_position(&self) -> u32 {
        self.parent_position
    }
}
impl NodeDetails {
    /// Returns the path of the node
    #[must_use]
    pub fn get_path(&self) -> PathRef<'_> {
        self.path.as_ref()
    }
    /// Returns `true` if the node is reachable from the spigot root
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.active
    }
    /// Returns the weight of the node (or `None` if all siblings are equal)
    #[must_use]
    pub fn get_weight(&self) -> Option<u32> {
        self.weight
    }
    /// Returns the ordering for the node's children
    #[must_use]
    pub fn get_order_type(&self) -> OrderType {
        self.order_type
    }
}
// NodeKind accessors
impl NodeDetails {
    /// Returns true if the node is a bucket
    #[must_use]
    pub fn is_bucket(&self) -> bool {
        self.get_bucket_item_count().is_some()
    }
    /// If the node is a bucket, returns the number of items in the bucket
    #[must_use]
    pub fn get_bucket_item_count(&self) -> Option<u32> {
        match self.kind {
            NodeKind::Bucket { item_count } => Some(item_count),
            NodeKind::Joint { .. } | NodeKind::JointAbbrev { .. } => None,
        }
    }
    /// If the node is a joint, returns the number of child nodes in the joint
    #[must_use]
    pub fn get_joint_child_count(&self) -> Option<u32> {
        match self.kind {
            NodeKind::Joint { child_count } | NodeKind::JointAbbrev { child_count } => {
                Some(child_count)
            }
            NodeKind::Bucket { .. } => None,
        }
    }
    /// Returns whether the node is a joint with children hidden in the view
    #[must_use]
    pub fn is_joint_children_hidden(&self) -> bool {
        matches!(self.kind, NodeKind::JointAbbrev { .. })
    }
}

impl TableView {
    pub(super) fn new(rows: Vec<Row>, total_width: u32) -> Self {
        #[cfg(debug_assertions)]
        Self::sanity_check_position_widths(&rows, total_width);

        Self { rows, total_width }
    }
    #[cfg(debug_assertions)]
    fn sanity_check_position_widths(rows: &[Row], total_width: u32) {
        // total_width
        debug_assert_eq!(
            total_width,
            rows.iter()
                .map(|row| row.get_cells().iter().map(Cell::get_display_width).sum())
                .max()
                .unwrap_or(0),
            "TableView total_width should equal the maximum row width"
        );

        // cell positions
        for row in rows {
            let mut position = 0;
            for cell in row.get_cells() {
                debug_assert_eq!(
                    position,
                    cell.get_position(),
                    "TableView cell position should match sum of prior widths"
                );
                position += cell.get_display_width();
            }
        }

        // cell parent_position
        for (row_index, row) in rows.iter().enumerate() {
            for cell in row.get_cells() {
                let cell_pos = cell.get_position();
                let parent_pos = cell.get_parent_position();
                debug_assert!(
                    cell_pos >= parent_pos,
                    "TableView cell position ({cell_pos}) should be at/after parent position ({parent_pos})"
                );

                if let Some(prev_row_index) = row_index.checked_sub(1) {
                    let previous_row = rows[prev_row_index].get_cells();
                    if let Ok(parent_index) =
                        previous_row.binary_search_by_key(&parent_pos, Cell::get_position)
                    {
                        let parent_cell = &previous_row[parent_index];
                        let parent_end =
                            parent_cell.get_position() + parent_cell.get_display_width();
                        if cell.get_display_width() == 0 {
                            debug_assert!(
                                cell_pos <= parent_end,
                                "TableView cell position ({cell_pos}) should be within/at parent end ({parent_end}) for zero-width cell"
                            );
                        } else {
                            debug_assert!(
                                cell_pos < parent_end,
                                "TableView cell position ({cell_pos}) should be within parent end ({parent_end})"
                            );
                        }
                    } else {
                        debug_assert!(false, "parent_position should match cell in previous row");
                    };
                }
            }
        }
    }
}

impl std::fmt::Display for NodeDetails {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use std::borrow::Cow;

        let Self {
            path,
            active,
            weight,
            kind,
            order_type,
        } = self;
        let kind_description = match kind {
            NodeKind::Bucket { item_count: 0 } => Cow::Borrowed("bucket (empty)"),
            NodeKind::Bucket { item_count: 1 } => Cow::Borrowed("bucket (1 item)"),
            NodeKind::Bucket {
                item_count: c @ 2..,
            } => Cow::Owned(format!("bucket ({c} items)")),
            //
            NodeKind::Joint { child_count: 0 } | NodeKind::JointAbbrev { child_count: 0 } => {
                Cow::Borrowed("joint (empty)")
            }
            NodeKind::Joint { child_count: 1 } => Cow::Borrowed("joint (1 child)"),
            NodeKind::JointAbbrev { child_count: 1 } => Cow::Borrowed("joint (1 child hidden)"),
            NodeKind::Joint {
                child_count: c @ 2..,
            } => Cow::Owned(format!("joint ({c} children)")),
            NodeKind::JointAbbrev {
                child_count: c @ 2..,
            } => Cow::Owned(format!("joint ({c} children hidden)")),
        };
        //
        write!(f, "{path} ")?;
        if let Some(weight) = weight {
            write!(f, "x{weight} ")?;
        }
        write!(f, "{kind_description} {order_type}")?;
        if !active {
            write!(f, " (inactive)")?;
        }
        Ok(())
    }
}

impl std::fmt::Display for TableView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let total_width = usize::try_from(self.get_max_row_width()).expect("u32 fits in usize");

        writeln!(f, "Table {{")?;
        // format as one cell per line, and use symbols to represent the nodes
        // e.g. "XXXXXXX <------- description"
        for row in self.get_rows() {
            let mut position = 0;

            for cell in row.get_cells() {
                let width = usize::try_from(cell.get_display_width()).expect("u32 fits in usize");
                let remainder_width = total_width - width - position;

                if let Some(node) = cell.get_node() {
                    let marker_char = if node.is_active() { 'X' } else { 'o' };
                    let marker: String = std::iter::repeat(marker_char).take(width).collect();
                    writeln!(
                        f,
                        "{:<position$}{marker} <{:-<remainder_width$}--- {node}",
                        "", "",
                    )?;
                } else if cell.get_display_width() == 0 {
                    let marker = "?";
                    writeln!(
                        f,
                        "{:<position$}{marker} <{:-<remainder_width$}--- (one or more nodes omitted...)",
                        "", "")?;
                }

                position += width;
            }
        }
        writeln!(f, "}}")
    }
}
impl std::fmt::Debug for TableView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self}")
    }
}
