// Copyright (C) 2021-2025  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Defines the application logic (ignoring all HTTP / websocket framework specifics)

use crate::websocket;
use bucket_spigot::{order::OrderType, path::Path};

mod typescript_bindings;

/// Runs the client commands until the channel is disconnected
pub fn app_logic(cmd_rx: std::sync::mpsc::Receiver<SpigotCommand>) {
    for cmd in cmd_rx {
        let SpigotCommand { kind, response } = cmd;
        let _ = response.send(app_logic_cmd(kind));
    }
}
fn app_logic_cmd(kind: SpigotCommandKind) -> SpigotResponse {
    match kind {
        SpigotCommandKind::Echo { message } => {
            let message = format!("Response to {message:?}");
            SpigotResponse::EchoResponse { message }
        }
        SpigotCommandKind::Network(network_modify_cmd) => todo!(), // TODO
    }
}

/// Command to be run, with a response to the client
///
/// See [`SpigotCommandKind`] for specific messages
pub struct SpigotCommand {
    kind: SpigotCommandKind,
    response: std::sync::mpsc::SyncSender<SpigotResponse>,
}
impl websocket::Command for SpigotCommand {
    type Inner = SpigotCommandKind;
    type Response = SpigotResponse;
    fn new(kind: SpigotCommandKind) -> (Self, std::sync::mpsc::Receiver<SpigotResponse>) {
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        (Self { kind, response: tx }, rx)
    }
}
/// Command from the client
#[derive(Debug, serde::Deserialize)]
#[serde(tag = "kind")]
pub enum SpigotCommandKind {
    /// Simple ping/pong check
    Echo {
        #[allow(missing_docs)]
        message: String,
    },
    /// Modify the `bucket_spigot` network
    Network(NetworkModifyCmd),
}
/// Response for a [`SpigotCommandKind`]
#[derive(Debug, serde::Serialize)]
#[serde(tag = "kind")]
pub enum SpigotResponse {
    /// Response for [`SpigotCommandKind::Echo`]
    EchoResponse {
        #[allow(missing_docs)]
        message: String,
    },
}

/// Subset of [`bucket_spigot::ModifyCmd`] for the client to control
#[allow(missing_docs)] // see linked source type for details
#[derive(Debug, serde::Deserialize)]
#[serde(tag = "cmd")]
pub enum NetworkModifyCmd {
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
impl From<NetworkModifyCmd> for bucket_spigot::ModifyCmd<String, String> {
    fn from(value: NetworkModifyCmd) -> Self {
        use NetworkModifyCmd as Local;
        match value {
            Local::AddBucket { parent } => Self::AddBucket { parent },
            Local::AddJoint { parent } => Self::AddJoint { parent },
            Local::DeleteEmpty { path } => Self::DeleteEmpty { path },
            Local::SetFilters { path, new_filters } => Self::SetFilters { path, new_filters },
            Local::SetWeight { path, new_weight } => Self::SetWeight { path, new_weight },
            Local::SetOrderType {
                path,
                new_order_type,
            } => Self::SetOrderType {
                path,
                new_order_type,
            },
        }
    }
}
