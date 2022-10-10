// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Handles [`sequencer`]-related items, named `seq` to avoid namespace ambiguity
// e.g. (create::sequencer vs ::sequencer)

use crate::cli;
use shared::Shutdown;
use tokio::sync::watch;

pub(crate) type Sequencer = sequencer::Sequencer<source::Source, SequencerFilter>;
pub(crate) type SequencerFilter = Option<source::TypedArg>;
pub(crate) type SequencerCommand = sequencer::command::Command<SequencerFilter>;

pub(crate) async fn sequencer_task(
    mut sequencer: Sequencer,
    mut sequencer_rx: tokio::sync::mpsc::Receiver<SequencerCommand>,
    cmd_playlist_tx: vlc_http::cmd_playlist_items::Sender,
    state_tx: watch::Sender<String>,
) -> Result<shared::Never, Shutdown> {
    let vlc_http::cmd_playlist_items::Sender {
        urls_tx,
        mut remove_rx,
    } = cmd_playlist_tx;
    loop {
        // publish state
        match serde_json::to_string_pretty(&sequencer.tree_serializable()) {
            Ok(tree_str) => {
                if let Err(send_err) = state_tx.send(tree_str) {
                    dbg!(send_err);
                }
            }
            Err(serde_json_err) => {
                dbg!(serde_json_err);
            }
        }
        tokio::select! {
            Some(command) = sequencer_rx.recv() => {
                let result = sequencer.run(command);
                if let Err(sequencer_err) = result {
                    // TODO include a oneshot receiver in the command, to signal success/failure message?
                    dbg!(sequencer_err);
                }
            }
            Ok(()) = remove_rx.changed() => {
                if let Some(removed) = &*remove_rx.borrow() {
                    let popped = sequencer.pop_next();
                    println!("remove_rx changed! removed {removed}, and popped {popped:?}");
                }
            }
            else => {
                break;
            }
        }
        // update cmd_playlist items
        let new_urls = sequencer
            .get_root_queue_items()
            .map(cli::parse_url)
            .collect();
        match new_urls {
            Ok(new_urls) => {
                urls_tx.send_modify(|data| {
                    data.items = new_urls;
                });
            }
            Err(url_err) => {
                dbg!(url_err);
            }
        }
    }
    Err(Shutdown)
}

pub use cmd::Cmd;
mod cmd {
    use super::{source, SequencerCommand};
    use q_filter_tree::{OrderType, Weight};

    #[derive(clap::Subcommand, Debug)]
    pub enum Cmd {
        //------ [Sequencer] ------
        /// Add a new node
        Add {
            /// Target parent path for the new node
            parent_path: String,
            /// Filter for the new node
            filter: Vec<String>,
            /// Type of the source for interpreting `filter`
            #[clap(long, arg_enum)]
            source_type: Option<source::Type>,
        },
        /// Add a new terminal node
        AddTerminal {
            /// Target parent path for the new terminal node
            parent_path: String,
            /// Filter for the new terminal node
            filter: Vec<String>,
            /// Type of the source for interpreting `filter`
            #[clap(long, arg_enum)]
            source_type: Option<source::Type>,
        },
        /// Set filter for an existing node
        SetFilter {
            /// Target node path
            path: String,
            /// New filter value
            filter: Vec<String>,
            /// Type of the source for interpreting `filter`
            #[clap(long, arg_enum)]
            source_type: Option<source::Type>,
        },
        /// Set weight of an item in a terminal node
        SetItemWeight {
            /// Target node path
            path: String,
            /// Index of the item to set
            item_index: usize,
            /// New weight value
            weight: Weight,
        },
        /// Set weight of a node
        SetWeight {
            /// Target node path
            path: String,
            /// New weight value
            weight: Weight,
        },
        /// Set ordering type of a node
        SetOrderType {
            /// Target node path
            path: String,
            /// New order type value
            #[clap(subcommand)]
            order_type: OrderType,
        },
        /// Update the items for all terminal nodes reachable from the specified parent
        Update {
            /// Target node path
            path: String,
        },
        /// Removes the specified node
        Remove {
            /// Target node id
            id: String,
        },
        /// Sets the minimum count of items to keep staged in the specified node's queue
        SetPrefill {
            /// Minimum number of items to stage
            min_count: usize,
            /// Target node path (default is root)
            path: Option<String>,
        },
        /// Removes an item from the queue of the specified node
        QueueRemove {
            /// Index of the queue item to remove
            index: usize,
            /// Path of the target node (default is root)
            path: Option<String>,
        },
    }
    impl From<Cmd> for SequencerCommand {
        #[rustfmt::skip] // too many extra line breaks if rustfmt is run
        fn from(cmd: Cmd) -> Self {
            // too cumbersome to grab into the main Config to find a default type,
            // so just define it here as a constant.  Cli usage ergonomics is lower priority.
            const DEFAULT_TY: source::Type = source::Type::Beet;
            match cmd {
                Cmd::Add { parent_path, filter, source_type } => {
                    let filter = parse_filter_args(DEFAULT_TY, filter, source_type);
                    sequencer::command::AddNode { parent_path, filter }.into()
                }
                Cmd::AddTerminal { parent_path, filter, source_type } => {
                    let filter = parse_filter_args(DEFAULT_TY, filter, source_type);
                    sequencer::command::AddTerminalNode { parent_path, filter }.into()
                }
                Cmd::SetFilter { path, filter, source_type } => {
                    let filter = parse_filter_args(DEFAULT_TY, filter, source_type);
                    sequencer::command::SetNodeFilter { path, filter }.into()
                }
                Cmd::SetItemWeight { path, item_index, weight } => {
                    sequencer::command::SetNodeItemWeight { path, item_index, weight }.into()
                }
                Cmd::SetWeight { path, weight } => {
                    sequencer::command::SetNodeWeight { path, weight }.into()
                }
                Cmd::SetOrderType { path, order_type } => {
                    sequencer::command::SetNodeOrderType { path, order_type }.into()
                }
                Cmd::Update { path } => {
                    sequencer::command::UpdateNodes { path }.into()
                }
                Cmd::Remove { id } => {
                    sequencer::command::RemoveNode { id }.into()
                }
                Cmd::SetPrefill { path, min_count } => {
                    sequencer::command::SetNodePrefill { path, min_count }.into()
                }
                Cmd::QueueRemove { path, index } => {
                    sequencer::command::QueueRemove { path, index }.into()
                }
            }
        }
    }

    fn parse_filter_args(
        default_ty: source::Type,
        items_filter: Vec<String>,
        source_type: Option<source::Type>,
    ) -> Option<source::TypedArg> {
        if items_filter.is_empty() {
            None
        } else {
            let joined = items_filter.join(" ");
            let filter = match source_type.unwrap_or(default_ty) {
                // source::Type::Debug => source::TypedArg::Debug(joined),
                source::Type::FileLines => source::TypedArg::FileLines(joined),
                source::Type::FolderListing => source::TypedArg::FolderListing(joined),
                source::Type::Beet => source::TypedArg::Beet(items_filter),
            };
            Some(filter)
        }
    }
}

pub mod source {
    use clap::ValueEnum;
    use sequencer::sources::{Beet, FileLines, FolderListing, RootFolder};
    use serde::Serialize;

    sequencer::source_multi_select! {
        #[derive(Clone)]
        pub struct Source {
            type Args = Args<'a>;
            #[derive(Copy, Clone, ValueEnum)]
            type Type = Type;
            /// Beet
            beet: Beet as Beet where arg type = Vec<String>,
            /// File lines
            file_lines: FileLines as FileLines where arg type = String,
            /// Folder listing
            folder_listing: FolderListing as FolderListing where arg type = String,
        }
        #[derive(Clone, Debug, Serialize)]
        /// Typed argument
        impl ItemSource<Option<TypedArg>> {
            type Item = String;
            /// Typed Error
            type Error = TypedLookupError;
        }
    }
    impl Source {
        pub(crate) fn new(root_folder: RootFolder, beet: Beet) -> Self {
            let file_lines = FileLines::from(root_folder.clone());
            let folder_listing = FolderListing::from(root_folder);
            Self {
                beet,
                file_lines,
                folder_listing,
            }
        }
    }
}
