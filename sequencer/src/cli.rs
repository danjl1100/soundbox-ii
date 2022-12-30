// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Command-line interface for `Sequencer`

use crate::{
    command::{self, Runnable},
    sources::ItemSource,
    Error, Sequencer,
};
use clap::Parser;
use q_filter_tree::{OrderType, Weight};

/// Command-line interface for `Sequencer`
pub struct Cli<T: ItemSource<Option<F>>, U: FilterArgParser<Filter = F>, F> {
    /// Sequencer
    pub sequencer: Sequencer<T, Option<F>>,
    filter_arg_parser: U,
    params: OutputParams,
}
/// Converter from `String` arguments into a client-specified filter type
pub trait FilterArgParser {
    /// Type marker for the filter (if any)
    type Type: for<'a> From<&'a Self::Filter>
        + std::fmt::Debug
        + Eq
        + clap::ValueEnum
        + Send
        + Sync
        + 'static;
    /// Filter type constructed from the arguments
    type Filter;
    /// Converts the specified arguments to a filter
    fn parse_filter_args(
        &self,
        args: Vec<String>,
        source_type: Option<Self::Type>,
    ) -> Option<Self::Filter>;
}
/// Cli parameters
pub struct OutputParams {
    /// Slience non-error output that is not explicitly requested
    pub quiet: bool,
}
impl OutputParams {
    /// Prints the specified information (unless quiet mode is set)
    pub fn output(&self, fmt_args: std::fmt::Arguments) {
        if !self.quiet {
            println!("{fmt_args}");
        }
    }
}

impl<T: ItemSource<Option<F>>, U, F> Cli<T, U, F>
where
    T::Item: std::fmt::Debug,
    U: FilterArgParser<Filter = F>,
    F: serde::Serialize + Clone + std::fmt::Debug,
{
    /// Constructs a Cli for the specified source, filter arg parser, and output parameters
    pub fn new(source: T, filter_arg_parser: U, params: OutputParams) -> Self {
        let sequencer: Sequencer<T, Option<F>> = Sequencer::new(source, None);
        Self {
            sequencer,
            filter_arg_parser,
            params,
        }
    }
    /// Executes the specified command
    ///
    /// # Errors
    /// Returns the `sequencer` Error, if any
    #[allow(clippy::too_many_lines)] // this is OK, breaking match cases into functions
                                     // would harm readability
    pub fn exec_command(&mut self, command: NodeCommand<U::Type>) -> Result<(), Error> {
        let _: OutputProof = match command {
            NodeCommand::Print => self.output_summary(format_args!("{}", &self.sequencer)),
            NodeCommand::Add {
                parent_path,
                items_filter,
                source_type: requested_type,
            } => {
                let source_type = self.calculate_existing_type(&parent_path, requested_type)?;
                let node_path = if let Some(filter) = self
                    .filter_arg_parser
                    .parse_filter_args(items_filter, source_type)
                {
                    self.run(command::AddTerminalNode {
                        parent_path,
                        filter: Some(filter),
                    })
                } else {
                    self.run(command::AddNode {
                        parent_path,
                        filter: None,
                    })
                }?;
                self.output_summary(format_args!("added node {node_path}"))
            }
            NodeCommand::SetFilter {
                path,
                items_filter,
                source_type: requested_type,
            } => {
                let source_type = self.calculate_existing_type(&path, requested_type)?;
                let filter = self
                    .filter_arg_parser
                    .parse_filter_args(items_filter, source_type);
                let filter_print = filter.clone();
                let old = self.run(command::SetNodeFilter { path, filter });
                self.output_summary(format_args!(
                    "changed filter from {old:?} -> {filter_print:?}"
                ))
            }
            NodeCommand::SetWeight {
                path,
                item_index,
                weight,
            } => {
                let old_weight = if let Some(item_index) = item_index {
                    self.run(command::SetNodeItemWeight {
                        path,
                        item_index,
                        weight,
                    })
                } else {
                    self.run(command::SetNodeWeight { path, weight })
                }?;
                self.output_summary(format_args!("changed weight from {old_weight} -> {weight}"))
            }
            NodeCommand::SetOrderType { path, order_type } => {
                let old = self.run(command::SetNodeOrderType { path, order_type })?;
                self.output_summary(format_args!(
                    "changed order type from {old:?} -> {order_type:?}"
                ))
            }
            NodeCommand::Update { path } => {
                let path = path.unwrap_or_else(|| ".".to_string());
                let path_print = path.clone();
                self.run(command::UpdateNodes { path })?;
                self.output_summary(format_args!("updated nodes under path {path_print}"))
            }
            NodeCommand::Remove { id } => {
                let id_print = id.clone();
                self.run(command::RemoveNode { id })?;
                // let removed = self.sequencer.remove_node(&id)?;
                // let (weight, info) = removed;
                self.output_summary(format_args!("removed node {id_print}"))
            }
            NodeCommand::SetPrefill { path, min_count } => {
                let path_str = path_clone_description(&path);
                self.run(command::SetNodePrefill { path, min_count })?;
                self.output_summary(format_args!("prefill set to {min_count} for {path_str}"))
            }
            NodeCommand::QueueRemove { index, path } => {
                let path_str = path_clone_description(&path);
                self.run(command::QueueRemove { path, index })?;
                self.output_summary(format_args!("removed index {index} from path {path_str}"))
            }
            NodeCommand::Move {
                src_id,
                dest_parent_id,
            } => {
                let src_id_str = src_id.to_string();
                let actual_dest = self.run(command::MoveNode {
                    src_id,
                    dest_parent_id,
                })?;
                self.output_summary(format_args!("moved node {src_id_str} to {actual_dest}"))
            }
        };
        Ok(())
    }
    fn run<V>(&mut self, command: V) -> Result<V::Output, Error>
    where
        V: Runnable<Option<F>>,
    {
        self.sequencer.run(command)
    }
    fn calculate_existing_type(
        &self,
        path: &str,
        requested_type: Option<U::Type>,
    ) -> Result<Option<U::Type>, Error> {
        self.sequencer
            .calculate_required_type(path, requested_type)?
            .map_err(|mismatch_label| format!("{mismatch_label}").into())
    }
    /// Prints the specified information (unless quiet mode is set)
    pub fn output(&self, fmt_args: std::fmt::Arguments) {
        self.output_summary(fmt_args);
    }
    fn output_summary(&self, fmt_args: std::fmt::Arguments) -> OutputProof {
        self.params.output(fmt_args);
        OutputProof
    }
}
/// Evidence that `output` was indeed called
struct OutputProof;
fn path_clone_description(path_opt: &Option<String>) -> String {
    path_opt.clone().unwrap_or_else(|| "root".to_string())
}

/// Cli command for the `sequencer`, for the given source-type `clap::ValueEnum`
#[derive(Parser, Debug)]
pub enum NodeCommand<T>
where
    T: clap::ValueEnum + Send + Sync + 'static,
{
    /// Print the current sequencer-nodes state
    Print,
    // TODO add granular print that accepts a path, with optional "recursive" flag
    /// Add a new node for items or fanning-out to child nodes
    Add {
        /// Path of the parent for the new node (use "." for the root node)
        parent_path: String,
        /// Filter value(s) for terminal nodes only (optional, default is non-terminal node)
        items_filter: Vec<String>,
        /// Type of the source (defaults to main-args option)
        #[clap(long, value_enum)]
        source_type: Option<T>,
    },
    /// Set the filter for the specified node
    SetFilter {
        /// Path of the node to modify
        path: String,
        /// New filter value
        items_filter: Vec<String>,
        /// Type of the source (defaults to application-specific option)
        #[clap(long, value_enum)]
        source_type: Option<T>,
    },
    /// Set the weight for the specified node or item
    SetWeight {
        /// Path of the node to modify
        path: String,
        /// Index of the item to modify (for terminal nodes only)
        #[clap(long)]
        item_index: Option<usize>,
        /// New weight value
        weight: Weight,
    },
    /// Set the order type for the specified node
    SetOrderType {
        /// Path of the node to modify
        path: String,
        /// Method of ordering
        #[clap(subcommand)]
        order_type: OrderType,
    },
    /// Update the items for all terminal nodes reachable from the specified parent node
    Update {
        /// Path of the target node to update (optional, default is all nodes)
        path: Option<String>,
    },
    /// Remove a node
    Remove {
        /// Id of the target node to delete
        id: String,
        //TODO is this appropriate?
        // recursive: bool,
    },
    /// Set the minimum number of staged (determined) items at the root node
    #[clap(alias("prefill"))]
    SetPrefill {
        /// Minimum number of items to stage
        min_count: usize,
        /// Path of the target node (default is root)
        path: Option<String>,
    },
    /// Removes an item from the queue of the specified node
    QueueRemove {
        /// Index of the queue item to remove
        index: usize,
        /// Path of the target node (default is root)
        path: Option<String>,
    },
    /// Moves a (non-root) node from one chain node to another
    Move {
        /// Id of the node to move (root is forbidden)
        src_id: String,
        /// Id of the existing destination node
        dest_parent_id: String,
    },
}
impl<T> std::fmt::Display for NodeCommand<T>
where
    T: clap::ValueEnum + Send + Sync + 'static + std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
