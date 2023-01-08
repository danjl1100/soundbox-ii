// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Handles [`sequencer`]-related items, named `seq` to avoid namespace ambiguity
// e.g. (create::sequencer vs ::sequencer)

use crate::{
    cli::{self, SequencerState},
    config,
};
use shared::Shutdown;
use tokio::sync::{mpsc, oneshot, watch};

pub(crate) type Sequencer = sequencer::Sequencer<source::Source, SequencerFilter>;
pub(crate) type SequencerFilter = Option<source::TypedArg>;
pub(crate) type SequencerCommand = sequencer::command::Command<SequencerFilter>;
pub(crate) type SequencerResult =
    Result<sequencer::command::TypedOutput<SequencerFilter>, sequencer::Error>;
pub(crate) type NodeCommand = sequencer::cli::NodeCommand<source::Type>;

pub(crate) struct SequencerAction(SequencerCommand, oneshot::Sender<SequencerResult>);
impl SequencerAction {
    pub fn new(cmd: SequencerCommand) -> (Self, oneshot::Receiver<SequencerResult>) {
        let (tx, rx) = oneshot::channel();
        (Self(cmd, tx), rx)
    }
    fn exec(self, sequencer: &mut Sequencer) {
        let Self(command, result_rx) = self;
        let result = sequencer.run(command);
        if let Err(unsent_result) = result_rx.send(result) {
            drop(dbg!(unsent_result));
        }
    }
}

pub(crate) struct Channels {
    pub cmd_playlist_tx: vlc_http::cmd_playlist_items::Sender,
    pub sequencer_state_tx: watch::Sender<cli::SequencerState>,
    pub sequencer_rx: mpsc::Receiver<SequencerAction>,
    pub sequencer_cli_rx: mpsc::Receiver<NodeCommand>,
}

pub(crate) struct Task {
    cli: sequencer::cli::Cli<source::Source, source::FilterArgParser, source::TypedArg>,
    channels: Channels,
}
impl Task {
    pub(crate) fn new(config: config::Sequencer, channels: Channels) -> Self {
        let cli = {
            let config::Sequencer {
                root_folder,
                beet_cmd,
            } = config;
            let item_source = source::Source::new(root_folder, beet_cmd);
            let filter_arg_parser = source::FilterArgParser {
                default_ty: source::Type::Beet, // TODO configurable? or no?
            };
            let params = sequencer::cli::OutputParams { quiet: false };
            sequencer::cli::Cli::new(item_source, filter_arg_parser, params)
        };
        Self { cli, channels }
    }
    pub(crate) async fn run(self) -> Result<shared::Never, Shutdown> {
        let Self { mut cli, channels } = self;
        let Channels {
            mut sequencer_rx,
            mut sequencer_cli_rx,
            cmd_playlist_tx,
            sequencer_state_tx,
        } = channels;
        let vlc_http::cmd_playlist_items::Sender {
            urls_tx,
            mut remove_rx,
        } = cmd_playlist_tx;
        loop {
            // publish state
            match serde_json::to_string_pretty(&cli.sequencer.tree_serializable()) {
                Ok(tree_str) => {
                    let new_state = SequencerState(tree_str);
                    if let Err(send_err) = sequencer_state_tx.send(new_state) {
                        dbg!(send_err);
                    }
                }
                Err(serde_json_err) => {
                    dbg!(serde_json_err);
                }
            }
            tokio::select! {
                Some(action) = sequencer_rx.recv() => {
                    action.exec(&mut cli.sequencer);
                }
                Some(command) = sequencer_cli_rx.recv() => {
                    let result = cli.exec_command(command);
                    match result {
                        Ok(()) => {}
                        Err(err) => eprintln!("Error: {err}"),
                    }
                }
                Ok(()) = remove_rx.changed() => {
                    if let Some(removed) = &*remove_rx.borrow() {
                        Self::exec_remove(removed, &mut cli.sequencer);
                    }
                }
                else => {
                    break;
                }
            }
            // update cmd_playlist items
            let new_urls = cli
                .sequencer
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
    fn exec_remove(removed: &url::Url, sequencer: &mut Sequencer) {
        println!("remove_rx changed! removed {removed}");
        let popped = sequencer.pop_next();
        if let Some(popped) = popped {
            let (node_seq, popped) = popped.into_parts();
            println!("\tpopped {popped:?} from node #{node_seq}");
        } else {
            println!("\tpopped None");
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

    pub(super) struct FilterArgParser {
        pub default_ty: Type,
    }
    impl sequencer::cli::FilterArgParser for FilterArgParser {
        type Type = Type;
        type Filter = TypedArg;

        fn parse_filter_args(
            &self,
            items_filter: Vec<String>,
            source_type: Option<Type>,
        ) -> Option<TypedArg> {
            if items_filter.is_empty() {
                None
            } else {
                let joined = items_filter.join(" ");
                let filter = match source_type.unwrap_or(self.default_ty) {
                    // Type::Debug => source::TypedArg::Debug(joined),
                    Type::FileLines => TypedArg::FileLines(joined),
                    Type::FolderListing => TypedArg::FolderListing(joined),
                    Type::Beet => TypedArg::Beet(items_filter),
                };
                Some(filter)
            }
        }
    }
}

// transparent, showing only command
impl std::fmt::Display for SequencerAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self(command, _) = self;
        write!(f, "{command:?}")
    }
}
