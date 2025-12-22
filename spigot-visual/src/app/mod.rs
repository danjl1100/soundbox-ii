// Copyright (C) 2021-2025  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Defines the application logic (ignoring all HTTP / websocket framework specifics)

use crate::websocket;
use bucket_spigot::{order::OrderType, path::Path};

mod typescript_bindings;

/// Runs the client commands until the channel is disconnected
///
/// # Errors
/// Returns an error if the `cmd_rx` source sends an error
pub fn app_logic(
    cmd_rx: std::sync::mpsc::Receiver<eyre::Result<SpigotCommand>>,
) -> eyre::Result<()> {
    AppLogic::new(cmd_rx)?.run()
}

struct AppLogic {
    cmd_rx: std::sync::mpsc::Receiver<eyre::Result<SpigotCommand>>,
    // TODO
    // state_file: StateFile,
    // spigot: Network<BeetItem, String>,
}
impl AppLogic {
    fn new(cmd_rx: std::sync::mpsc::Receiver<eyre::Result<SpigotCommand>>) -> eyre::Result<Self> {
        // let (state_file, _script) = StateFile::new("state.txt".into())?;
        if false {
            eyre::bail!("")
        }

        // let spigot = Self::create_spigot(script)?;
        let this = Self {
            cmd_rx,
            // state_file,
            // spigot,
        };
        // this.update_spigot()?;
        Ok(this)
    }
    // TODO - replace this, use beet-pusher instead!
    // fn create_spigot(script: Option<String>) -> Network<BeetItem, String> {
    //     let network = if let Some(script) = script {
    //         let mut network = Network::default();
    //         for line in script.lines() {
    //             let line = line.trim();
    //             if line.is_empty() || line.starts_with('#') {
    //                 continue;
    //             }

    //             let modify_cmd = serde_json::from_str(line)
    //                 .with_context(|| format!("invalid line in state file: {line:?}"))?;
    //             network.modify(modify_cmd)?;
    //         }
    //         network
    //     } else {
    //         // NOTE: **DO NOT** quote arguments, as there is no interpreter to strip the quotes
    //         const DEFAULT_SCRIPT: &str = "
    //             add-joint .

    //             add-bucket .0
    //             set-order-type .0.0 shuffle
    //             set-filters .0.0 added:2020.. grouping::^$

    //             add-bucket .0
    //             set-order-type .0.1 shuffle
    //             set-filters .0.1 grouping::1|2|3|4|5 has_lyrics::^$
    //             ";
    //         Network::from_commands_str_whitespace(DEFAULT_SCRIPT)?
    //     };

    //     Ok(network)
    // }
    // fn update_spigot(&mut self) -> eyre::Result<()> {
    //     // TODO move beet-pusher function to a common beet-spigot lib crate
    //     Ok(())
    // }
    fn run(self) -> eyre::Result<()> {
        for cmd in self.cmd_rx {
            let SpigotCommand { kind, response } = cmd?;
            let _ = response.send(app_logic_cmd(kind)?);

            // self.update_spigot()?;

            // {
            //     use std::fmt::Write as _;

            //     let mut lines = String::new();
            //     // FIXME horribly inefficient (multiple collects, at each stage)
            //     for cmd in self.spigot.serialize_collect() {
            //         let cmd = serde_json::to_string(&cmd)
            //             .with_context(|| format!("failed to serialize {cmd:?}"))?;
            //         writeln!(&mut lines, "{cmd}").expect("infallible");
            //     }
            //     self.state_file.overwrite(&lines)?;
            // }
        }
        Ok(())
    }
}
fn app_logic_cmd(kind: SpigotCommandKind) -> eyre::Result<SpigotResponse> {
    let response = match kind {
        SpigotCommandKind::Echo { message } => {
            let message = format!("Response to {message:?}");
            SpigotResponse::EchoResponse { message }
        }
        SpigotCommandKind::Network(_network_modify_cmd) => todo!(), // TODO
        SpigotCommandKind::Shutdown => {
            eyre::bail!("client requested shutdown")
        }
    };
    Ok(response)
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
    /// Exits the server (for testing purposes)
    Shutdown,
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

// TODO
// mod state_file {
//     use eyre::Context as _;
//     use std::path::PathBuf;
//
//     pub struct StateFile {
//         path: PathBuf,
//     }
//     impl StateFile {
//         pub fn new(path: PathBuf) -> eyre::Result<(Self, Option<String>)> {
//             let this = StateFile { path };
//             let loaded = this.load()?;
//             Ok((this, loaded))
//         }
//         pub fn load(&self) -> eyre::Result<Option<String>> {
//             let Self { path, .. } = self;
//             std::fs::read_to_string(path).map(Some).or_else(|e| {
//                 if e.kind() == std::io::ErrorKind::NotFound {
//                     Ok(None)
//                 } else {
//                     Err(e).with_context(|| format!("failed to read state file {}", path.display()))
//                 }
//             })
//         }
//         pub fn overwrite(&self, value: &str) -> eyre::Result<()> {
//             let Self { path, .. } = self;
//             std::fs::write(path, value)
//                 .with_context(|| format!("failed to write state file {}", path.display()))
//         }
//     }
// }
