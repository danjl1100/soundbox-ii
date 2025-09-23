// Copyright (C) 2021-2025  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Typescript versions of [`super::TableView`] and all contained fields
#![expect(dead_code)]

#[derive(ts_rs::TS)]
#[ts(export)]
struct TableView {
    rows: Vec<Row>,
    total_width: u32,
}
#[derive(ts_rs::TS)]
#[ts(export)]
struct Row(Vec<Cell>);

#[derive(ts_rs::TS)]
#[ts(export)]
struct Cell {
    display_width: u32,
    position: u32,
    parent_position: u32,
    node: Option<NodeDetails>,
}

#[derive(ts_rs::TS)]
#[ts(export)]
struct NodeDetails {
    path: Path,
    active: bool,
    weight: Option<u32>,
    kind: NodeKind,
    order_type: OrderType,
}

#[derive(ts_rs::TS)]
#[ts(export)]
enum NodeKind {
    Bucket { item_count: u32 },
    Joint { child_count: u32 },
    JointAbbrev { child_count: u32 },
}

#[derive(ts_rs::TS)]
#[ts(export)]
enum OrderType {
    InOrder,
    Random,
    Shuffle,
}

#[derive(ts_rs::TS)]
#[ts(export)]
struct Path(String);

mod construction_proof {
    //! Proof that the typescript types map one-to-one with the real types

    mod orig {
        pub(super) use super::super::super::{Cell, NodeDetails, Row, TableView};
        pub(super) use crate::order::OrderType;
        pub(super) use crate::path::PathRef;
        pub(super) use crate::view::NodeKind;
    }
    use super::{Cell, NodeDetails, NodeKind, OrderType, Path, Row, TableView};

    impl From<orig::TableView> for TableView {
        fn from(value: orig::TableView) -> Self {
            let orig::TableView { rows, total_width } = value;
            let rows = rows.into_iter().map(Into::into).collect();
            Self { rows, total_width }
        }
    }
    impl From<orig::Row> for Row {
        fn from(value: orig::Row) -> Self {
            let orig::Row(cells) = value;
            let cells = cells.into_iter().map(Into::into).collect();
            Self(cells)
        }
    }
    impl From<orig::Cell> for Cell {
        fn from(value: orig::Cell) -> Self {
            let orig::Cell {
                display_width,
                position,
                parent_position,
                node,
            } = value;
            Self {
                display_width,
                position,
                parent_position,
                node: node.map(Into::into),
            }
        }
    }
    impl From<orig::NodeDetails> for NodeDetails {
        fn from(value: orig::NodeDetails) -> Self {
            let orig::NodeDetails {
                path,
                active,
                weight,
                kind,
                order_type,
            } = value;
            Self {
                path: Path::from_path(path.as_ref()),
                active,
                weight,
                kind: kind.into(),
                order_type: order_type.into(),
            }
        }
    }
    impl From<orig::NodeKind> for NodeKind {
        fn from(value: orig::NodeKind) -> Self {
            match value {
                orig::NodeKind::Bucket { item_count } => Self::Bucket { item_count },
                orig::NodeKind::Joint { child_count } => Self::Joint { child_count },
                orig::NodeKind::JointAbbrev { child_count } => Self::JointAbbrev { child_count },
            }
        }
    }
    impl From<orig::OrderType> for OrderType {
        fn from(value: orig::OrderType) -> Self {
            match value {
                orig::OrderType::InOrder => Self::InOrder,
                orig::OrderType::Random => Self::Random,
                orig::OrderType::Shuffle => Self::Shuffle,
            }
        }
    }
    impl Path {
        fn from_path(value: orig::PathRef<'_>) -> Self {
            Self(value.to_string())
        }
    }
}
