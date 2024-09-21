// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

//! Views for a [`Network`]

use crate::{
    child_vec::{ChildVec, Weights},
    order::{OrderNode, OrderType, UnknownOrderPath},
    path::{Path, PathRef},
    Child, Network, UnknownPath,
};
use std::rc::Rc;

/// Tabular view of a [`Network`]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[allow(clippy::module_name_repetitions)]
#[must_use]
pub struct TableView {
    rows: Vec<Row>,
    total_width: u32,
}
/// Sequence of [`Cell`]s at the same depth
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Row(Vec<Cell>);

/// There are three kinds of `Cell`:
///
///    1. Node, when: `display_width > 0`, `node = Some(_)`
///    2. Spacer, when: `display_width > 0`, `node = None`
///    3. Horizontal continuation marker (column width-wise), when: `display_width = 0`, `node = None`
///
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Cell {
    display_width: u32,
    node: Option<NodeDetails>,
}

impl TableView {
    /// Returns the [`Row`]s in the table
    #[must_use]
    pub fn get_rows(&self) -> &[Row] {
        &self.rows
    }
}
impl Row {
    /// Returns the [`Cell`]s in the row
    #[must_use]
    pub fn get_cells(&self) -> &[Cell] {
        &self.0
    }
    fn push(&mut self, cell: Cell) {
        self.0.push(cell);
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
}
impl NodeDetails {
    /// Returns the path of the node
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

/// Details for a node
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct NodeDetails {
    path: Path,
    /// True if the node is reachable from the spigot root
    active: bool,
    /// Weight of the node relative to siblings (or `None` if all equal)
    weight: Option<u32>,
    kind: NodeKind,
    order_type: OrderType,
    // NOTE: exclude Filters list as it is relatively unbounded
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
enum NodeKind {
    /// Bucket node
    Bucket { item_count: u32 },
    /// Joint node
    Joint { child_count: u32 },
    /// Vertical continuation marker (row depth-wise) - joint node with
    /// children are hidden by `max_depth`
    JointAbbrev { child_count: u32 },
}

impl<T, U> Network<T, U> {
    /// Creates a [`TableView`]
    ///
    /// NOTE: each resulting node is either {Path/Id, Kind} or # omitted child nodes
    ///
    /// # Errors
    /// Returns an error if the specified path is incorrect, or the view dimensions are too large
    pub fn view_table(&self, table_params: TableParams<'_>) -> Result<TableView, ViewError> {
        let mut rows = vec![];
        let mut path = Path::empty();

        let mut item_node = &self.root;
        let mut order_node = self.root_order.node().get_children();
        let mut parent_active = true;
        let mut child_start_index = None;
        if let Some(base_path) = table_params.base_path {
            let parent_path = if let Some((child, parent)) = base_path.split_last() {
                child_start_index = Some(child);
                parent
            } else {
                base_path
            };
            for index in parent_path {
                path.push(index);
                parent_active =
                    parent_active && item_node.weights().map_or(false, |w| w.get(index) != 0);
                item_node = match item_node.children().get(index) {
                    Some(Child::Joint(joint)) => Ok(&joint.next),
                    Some(Child::Bucket(_)) | None => {
                        Err(crate::UnknownPath(base_path.clone_inner()))
                    }
                }?;
                order_node = match order_node.get(index) {
                    Some(node) => Ok(node.get_children()),
                    None => Err(crate::order::UnknownOrderPath(base_path.clone_inner())),
                }?;
            }
        }

        let total_width = table_params.find_child_nodes(
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
        )?;

        Ok(TableView { rows, total_width })
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

        {
            let dest_row = dest_cells.get_mut(state.depth).expect("row pushed above");
            let assumed_start = dest_row
                .get_cells()
                .iter()
                .map(|cell| cell.display_width)
                .sum();
            assert!(assumed_start <= state.position);
            match state.position.checked_sub(assumed_start) {
                Some(gap_width) if gap_width > 0 => {
                    dest_row.push(Cell {
                        display_width: gap_width,
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

        let start_position = state.position;
        let mut state = state;
        for ((index, child), order) in item_and_order {
            if matches!(self.max_width, Some(max_width) if state.position >= max_width) {
                let dest_row = dest_cells
                    .get_mut(state.depth)
                    .expect("row pushed by caller, above");
                dest_row.push(Cell {
                    display_width: 0,
                    node: None,
                });
                break;
            }

            // START - push index
            path_buf.push(index);

            let display_width = self.add_child_nodes(
                dest_cells,
                path_buf,
                state,
                weights,
                ((index, child), order),
            )?;

            state.position += display_width;

            // END - pop index
            path_buf.pop();
        }
        let total_width = state.position - start_position;
        Ok(total_width)
    }
    fn add_child_nodes<'a, T, U>(
        self,
        dest_cells: &mut Vec<Row>,
        path_buf: &mut Path,
        State {
            depth,
            position,
            parent_active,
        }: State,
        weights: Option<Weights<'_>>,
        ((index, child), order): ((usize, &'a Child<T, U>), &'a Rc<OrderNode>),
    ) -> Result<u32, ViewError>
    where
        T: 'a,
        U: 'a,
    {
        let weight = match weights {
            Some(weights) if weights.is_unity() => None,
            Some(weights) => Some(weights.get(index)),
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
            node: Some(node_details),
        });

        Ok(display_width)
    }
}
fn count(label: &'static str, count: usize) -> Result<u32, ExcessiveViewDimensions> {
    count
        .try_into()
        .map_err(|_| ExcessiveViewDimensions { label, count })
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
/// Owned version of [`TableParams`] for use in serializing view requests
pub struct TableParamsOwned {
    max_depth: Option<u32>,
    max_width: Option<u32>,
    base_path: Option<Path>,
    // TODO add a max node count... to autodetect the depth based on how many total cells are seen
}
/// Parameters for constructing a table view
#[derive(Clone, Copy, Debug, Default)]
#[must_use]
pub struct TableParams<'a> {
    max_depth: Option<u32>,
    max_width: Option<u32>,
    base_path: Option<PathRef<'a>>,
}
#[allow(unused)]
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
            base_path: self.base_path.as_ref().map(Path::as_ref),
        }
    }
    // Modify functions for non-`Copy` types only
    /// Replaces the owned base [`Path`]
    pub fn set_base_path(&mut self, base_path: Path) {
        self.base_path.replace(base_path);
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
        self.base_path.replace(base_path);
        self
    }
    /// Returns an owned version of the fields (cloning [`PathRef`] if any is set)
    pub fn to_owned(self) -> TableParamsOwned {
        let Self {
            max_depth,
            max_width,
            base_path,
        } = self;
        TableParamsOwned {
            max_depth,
            max_width,
            base_path: base_path.map(PathRef::clone_inner),
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
        let total_width = usize::try_from(self.total_width).expect("u32 fits in usize");

        writeln!(f, "Table {{")?;
        // format as one cell per line, and use symbols to represent the nodes
        // e.g. "XXXXXXX <------- description"
        for row in self.get_rows() {
            let mut position = 0;

            for cell in row.get_cells() {
                let width = usize::try_from(cell.display_width).expect("u32 fits in usize");
                let remainder_width = total_width - width - position;

                if let Some(node) = &cell.node {
                    let marker_char = if node.active { 'X' } else { 'o' };
                    let marker: String = std::iter::repeat(marker_char).take(width).collect();
                    writeln!(
                        f,
                        "{:<position$}{marker} <{:-<remainder_width$}--- {node}",
                        "", "",
                    )?;
                } else if cell.display_width == 0 {
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

/// Error modifying the [`Network`]
#[allow(clippy::module_name_repetitions)]
pub struct ViewError(ViewErr);
enum ViewErr {
    UnknownPath(UnknownPath),
    UnknownOrderPath(UnknownOrderPath),
    ExcessiveViewDimensions(ExcessiveViewDimensions),
}
impl From<UnknownPath> for ViewError {
    fn from(value: UnknownPath) -> Self {
        Self(ViewErr::UnknownPath(value))
    }
}
impl From<UnknownOrderPath> for ViewError {
    fn from(value: UnknownOrderPath) -> Self {
        Self(ViewErr::UnknownOrderPath(value))
    }
}
impl From<ExcessiveViewDimensions> for ViewError {
    fn from(value: ExcessiveViewDimensions) -> Self {
        Self(ViewErr::ExcessiveViewDimensions(value))
    }
}
impl From<ViewErr> for ViewError {
    fn from(value: ViewErr) -> Self {
        Self(value)
    }
}

struct ExcessiveViewDimensions {
    label: &'static str,
    count: usize,
}
impl std::fmt::Display for ExcessiveViewDimensions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { label, count } = self;
        write!(f, "excessive view dimensions ({label} {count})")
    }
}

impl std::error::Error for ViewError {}
impl std::fmt::Display for ViewError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self(inner) = self;
        match inner {
            ViewErr::UnknownPath(err) => write!(f, "{err}"),
            ViewErr::UnknownOrderPath(err) => {
                write!(f, "{err}")
            }
            ViewErr::ExcessiveViewDimensions(err) => write!(f, "{err}"),
        }
    }
}
impl std::fmt::Debug for ViewError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ViewError({self})")
    }
}
