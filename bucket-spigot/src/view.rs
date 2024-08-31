// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use crate::{
    child_vec::ChildVec,
    order::{OrderNode, OrderType, UnknownOrderPath},
    path::{Path, PathRef},
    Child, Network, UnknownPath,
};
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[allow(clippy::module_name_repetitions)]
#[must_use]
pub struct TableView {
    cells: Vec<Vec<Cell>>,
    total_width: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
// TODO for ease of serialized use, change to struct with display_width and optional Path, NodeDetails
pub enum Cell {
    Empty { gap_width: u32 },
    Node(Path, NodeDetails),
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct NodeDetails {
    /// True if the node is reachable from the spigot root
    active: bool,
    /// Weight of the node relative to siblings (or `None` if all equal)
    weight: Option<u32>,
    // Sum of the child nodes count
    display_width: u32,
    // display_position: u32,
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
    /// Joint node with children that are hidden by `max_depth`
    JointAbbrev { child_count: u32 },
}

impl<T, U> Network<T, U>
where
    T: std::fmt::Debug,
    U: std::fmt::Debug,
{
    /// Returns a tabular view
    ///
    /// NOTE: each resulting node is either {Path/Id, Kind} or # omitted child nodes
    ///
    /// # Errors
    /// Returns an error if the specified path is incorrect, or the view dimensions are too large
    pub fn view_table(&self, table_params: TableParamsRef<'_>) -> Result<TableView, ViewError> {
        let mut cells = vec![];
        let mut path = Path::empty();

        let mut item_node = &self.root;
        let mut order_node = self.root_order.node().get_children();
        if let Some(base_path) = table_params.base_path {
            for index in base_path {
                path.push(index);
                item_node = match item_node.children().get(index) {
                    Some(Child::Bucket(_)) => todo!(),
                    Some(Child::Joint(joint)) => Ok(&joint.next),
                    None => Err(crate::UnknownPath(base_path.clone_inner())),
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
            &mut cells,
            &mut path,
            State {
                depth: 0,
                start_position: 0,
                parent_active: true,
            },
        )?;

        Ok(TableView { cells, total_width })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct State {
    depth: usize,
    start_position: u32,
    parent_active: bool,
}
impl TableParamsRef<'_> {
    fn find_child_nodes<T, U>(
        self,
        item_nodes: &ChildVec<Child<T, U>>,
        order_nodes: &[Rc<OrderNode>],
        dest_cells: &mut Vec<Vec<Cell>>,
        path_buf: &mut Path,
        State {
            depth,
            start_position,
            parent_active,
        }: State,
    ) -> Result<u32, ViewError> {
        fn count(label: &'static str, count: usize) -> Result<u32, ExcessiveViewDimensions> {
            count
                .try_into()
                .map_err(|_| ExcessiveViewDimensions { label, count })
        }

        let Some(item_nodes_max_index) = item_nodes.len().checked_sub(1) else {
            return Ok(1);
        };

        assert!(dest_cells.len() >= depth);
        if dest_cells.len() == depth {
            // add column for this depth
            dest_cells.push(vec![]);
        }
        assert!(dest_cells.len() > depth);

        let weights = item_nodes.weights();
        if let Some(weights) = &weights {
            assert_eq!(weights.get_max_index(), item_nodes_max_index);
        }

        {
            let dest_column = dest_cells.get_mut(depth).expect("column pushed above");
            let assumed_start = dest_column.iter().map(Cell::get_display_width).sum();
            assert!(assumed_start <= start_position);
            match start_position.checked_sub(assumed_start) {
                Some(gap_width) if gap_width > 0 => {
                    dest_column.push(Cell::Empty { gap_width });
                }
                _ => {}
            }
        }

        let item_nodes = item_nodes.children();
        assert_eq!(item_nodes.len(), order_nodes.len());
        let mut total_width = 0;
        let mut display_position = start_position;
        for ((index, child), order) in item_nodes.iter().enumerate().zip(order_nodes) {
            path_buf.push(index);

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
            let order_type = order.get_order_type();
            let active = parent_active && weight.map_or(true, |w| w != 0);

            let child_width = if let Some((item_nodes, order_nodes)) = recurse {
                self.find_child_nodes(
                    item_nodes,
                    order_nodes,
                    dest_cells,
                    path_buf,
                    State {
                        depth: depth + 1,
                        start_position: display_position,
                        parent_active: active,
                    },
                )?
            } else {
                1
            };
            let display_width = child_width;
            total_width += display_width;

            let dest_column = dest_cells.get_mut(depth).expect("column pushed above");
            // let display_position = dest_column.last().map_or(start_position, |node| {
            //     node.display_position + node.display_width
            // });
            dest_column.push(Cell::Node(
                path_buf.clone(),
                NodeDetails {
                    active,
                    weight,
                    display_width,
                    // display_position,
                    kind,
                    order_type,
                },
            ));
            display_position += display_width;

            path_buf.pop();
        }

        Ok(total_width)
    }
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
/// Owned version of [`TableParamsRef`] for use in serializing view requests
pub struct TableParams {
    max_depth: Option<u32>,
    base_path: Option<Path>,
    // TODO add a max_width...
    // TODO add a max node count... to autodetect the depth based on how many total cells are seen
}
#[derive(Clone, Copy, Debug, Default)]
pub struct TableParamsRef<'a> {
    max_depth: Option<u32>,
    base_path: Option<PathRef<'a>>,
}
// TODO remove all modification functions on TableParams, modify through TableParamsRef instead,
// and only use this type for "Owned" serialize/deserialize
#[allow(unused)]
impl TableParams {
    pub fn max_depth(mut self, max_depth: u32) -> Self {
        self.max_depth.replace(max_depth);
        self
    }
    pub fn base_path(mut self, base_path: Path) -> Self {
        self.base_path.replace(base_path);
        self
    }
    pub fn as_ref(&self) -> TableParamsRef<'_> {
        TableParamsRef {
            max_depth: self.max_depth,
            base_path: self.base_path.as_ref().map(|path| path.as_ref()),
        }
    }
}
impl<'a> TableParamsRef<'a> {
    pub fn max_depth(mut self, max_depth: u32) -> Self {
        self.max_depth.replace(max_depth);
        self
    }
    pub fn base_path(mut self, base_path: PathRef<'a>) -> Self {
        self.base_path.replace(base_path);
        self
    }
}

// TODO delete if unused, NodeDetails::get_display_width
// impl NodeDetails {
//     fn get_display_width(&self) -> usize {
//         match self.kind {
//             NodeKind::Bucket { .. } | NodeKind::JointHasContinuation { .. } => 1,
//             NodeKind::Joint { child_count } => {
//                 usize::try_from(child_count).expect("joint child count fits in usize")
//             }
//         }
//     }
// }
impl Cell {
    fn get_display_width(&self) -> u32 {
        match self {
            Cell::Empty { gap_width } => *gap_width,
            Cell::Node(_, node) => node.display_width,
        }
    }
}
impl std::fmt::Display for NodeDetails {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use std::borrow::Cow;

        let Self {
            active,
            weight,
            display_width: _,
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
        // for row in &self.cells {
        //     for cell in row {
        //         match cell {
        //             Cell::Empty { gap_width } => {
        //                 eprintln!("[{gap_width}] empty");
        //             }
        //             Cell::Node(path, node) => {
        //                 let NodeDetails {
        //                     weight,
        //                     display_width,
        //                     // display_position,
        //                     kind,
        //                     order_type,
        //                 } = node;
        //                 eprintln!("[+{display_width}] {path} x{weight} {kind:?} {order_type:?}");
        //             }
        //         }
        //     }
        //     eprintln!("---------");
        // }

        writeln!(f, "Table {{")?;
        // format as one cell per line, and use symbols to represent the nodes
        // e.g. "XXXXXXX <------- description"
        for row in &self.cells {
            let mut position = 0;
            for cell in row {
                match cell {
                    Cell::Empty { gap_width } => {
                        position += usize::try_from(*gap_width).expect("u32 fits in usize");
                    }
                    Cell::Node(path, node) => {
                        let width = usize::try_from(node.display_width).expect("u32 fits in usize");
                        // let position = usize::try_from(*display_position).expect("u32 fits in usize");
                        let remainder_width = total_width - width - position;
                        // writeln!(
                        //     f,
                        //     "{:<position$}{:X<width$} <{:-<remainder_width$}--- {path} {node}",
                        //     "", "", ""
                        // )?;
                        let marker_char = if node.active { 'X' } else { 'o' };
                        let marker: String = std::iter::repeat(marker_char).take(width).collect();
                        writeln!(
                            f,
                            "{:<position$}{marker} <{:-<remainder_width$}--- {path} {node}",
                            "", "",
                        )?;

                        position += width;
                    }
                }
            }
            // writeln!(f, "{:=<50}", "")?;
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
