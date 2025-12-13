// Copyright (C) 2021-2025  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Typescript versions of [`super::TableView`] and all contained fields
#![expect(dead_code)]

#[derive(ts_rs::TS)]
#[ts(export)]
enum SpigotCommandKind {
    Echo { message: String },
    Network(NetworkModifyCmd),
}
#[derive(ts_rs::TS)]
#[ts(export)]
enum SpigotResponse {
    EchoResponse { message: String },
}

/// Subset of [`bucket_spigot::ModifyCmd`] for the client to control
#[derive(ts_rs::TS)]
#[ts(export)]
enum NetworkModifyCmd {
    AddBucket {
        parent: Path,
    },
    AddJoint {
        parent: Path,
    },
    DeleteEmpty {
        path: Path,
    },
    SetFilters {
        path: Path,
        new_filters: Vec<String>, // TODO is `String` OK?
    },
    SetWeight {
        path: Path,
        new_weight: u32,
    },
    SetOrderType {
        path: Path,
        new_order_type: OrderType,
    },
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

mod construction_proof_output {
    //! Proof that the typescript types map as an OUTPUT of the the real types

    use super::SpigotResponse;
    use crate::app as orig;

    impl From<orig::SpigotResponse> for SpigotResponse {
        fn from(value: orig::SpigotResponse) -> Self {
            use orig::SpigotResponse as Remote;
            match value {
                Remote::EchoResponse { message } => Self::EchoResponse { message },
            }
        }
    }
}

mod construction_proof_input {
    //! Proof that the typescript types map as an INPUT of the the real types
    use super::{NetworkModifyCmd, OrderType, Path, SpigotCommandKind};
    use crate::app as orig;

    impl TryFrom<SpigotCommandKind> for orig::SpigotCommandKind {
        type Error = ();
        fn try_from(value: SpigotCommandKind) -> Result<Self, Self::Error> {
            use SpigotCommandKind as Local;
            let converted = match value {
                Local::Echo { message } => Self::Echo { message },
                Local::Network(inner) => Self::Network(inner.try_into()?),
            };
            Ok(converted)
        }
    }

    impl TryFrom<NetworkModifyCmd> for orig::NetworkModifyCmd {
        type Error = ();
        fn try_from(value: NetworkModifyCmd) -> Result<Self, Self::Error> {
            use NetworkModifyCmd as Local;
            let converted = match value {
                Local::AddBucket { parent } => Self::AddBucket {
                    parent: parent.try_into()?,
                },
                Local::AddJoint { parent } => Self::AddJoint {
                    parent: parent.try_into()?,
                },
                Local::DeleteEmpty { path } => Self::DeleteEmpty {
                    path: path.try_into()?,
                },
                Local::SetFilters { path, new_filters } => Self::SetFilters {
                    path: path.try_into()?,
                    new_filters,
                },
                Local::SetWeight { path, new_weight } => Self::SetWeight {
                    path: path.try_into()?,
                    new_weight,
                },
                Local::SetOrderType {
                    path,
                    new_order_type,
                } => Self::SetOrderType {
                    path: path.try_into()?,
                    new_order_type: new_order_type.into(),
                },
            };
            Ok(converted)
        }
    }

    impl From<OrderType> for orig::OrderType {
        fn from(value: OrderType) -> Self {
            match value {
                OrderType::InOrder => Self::InOrder,
                OrderType::Random => Self::Random,
                OrderType::Shuffle => Self::Shuffle,
            }
        }
    }

    impl TryFrom<Path> for orig::Path {
        type Error = ();
        fn try_from(value: Path) -> Result<Self, ()> {
            let Path(s) = value;
            s.parse().map_err(|_| ())
        }
    }
}
