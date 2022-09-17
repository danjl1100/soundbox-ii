// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Command types for running a [`Sequencer`]

use q_filter_tree::{OrderType, Weight};

use crate::{sources::ItemSource, Error, NodeIdStr, Sequencer};

type Success = ();
command_enum! {
    /// Operations to perform on a [`Sequencer`]
    pub enum Command<'a, F>
    where
        F: Clone,
    {
        /// Add a new node
        AddNode<'a, F> -> NodeIdStr {
            /// Target parent path for the new node
            parent_path: &'a str,
            /// Filter for the new node
            filter: F,
        },
        /// Add a new terminal node
        AddTerminalNode<'a, F> -> NodeIdStr {
            /// Target parent path for the new terminal node
            parent_path: &'a str,
            /// Filter for the new terminal node
            filter: F,
        },
        /// Set filter for an existing node
        SetNodeFilter<'a, F> -> Filter {
            /// Target node path
            path: &'a str,
            /// New filter value
            filter: F,
        },
        /// Set weight of an item in a terminal node
        SetNodeItemWeight<'a> -> Weight {
            /// Target node path
            path: &'a str,
            /// Index of the item to set
            item_index: usize,
            /// New weight value
            weight: Weight,
        },
        /// Set weight of a node
        SetNodeWeight<'a> -> Weight {
            /// Target node path
            path: &'a str,
            /// New weight value
            weight: Weight,
        },
        /// Set ordering type of a node
        SetNodeOrderType<'a> -> OrderType {
            /// Target node path
            path: &'a str,
            /// New order type value
            order_type: OrderType,
        },
        /// Update the items for all terminal nodes reachable from the specified parent
        UpdateNodes<'a> -> Success {
            /// Target node path
            path: &'a str,
        },
        /// Removes the specified node
        RemoveNode<'a> -> Success {
            /// Target node id
            id: &'a str,
        },
    }
    mod out {
        /// Typed outputs
        pub enum Typed<F> {
            /// Id for a node
            NodeIdStr(crate::NodeIdStr),
            /// Filter
            Filter<F>(F),
            /// Weight
            Weight(q_filter_tree::Weight),
            /// OrderType
            OrderType(q_filter_tree::OrderType),
            /// No information, just success
            Success(()),
        }
    }
}
/// Runnable action by a [`Sequencer`]
pub trait Runnable<F> {
    /// Output of the action
    type Output;
    /// Execute the action
    ///
    /// # Errors
    /// Returns an [`Error`] when inputs do not match the inner tree state
    fn run<T>(self, sequencer: &mut Sequencer<T, F>) -> Result<Self::Output, Error>
    where
        T: ItemSource<F>,
        F: Clone;
}

command_runnable! {
    impl<'a, F> Runnable<F> for AddNode<'a, F> {
        fn run(self, seq) -> Result<NodeIdStr, Error> {
            let Self {
                parent_path,
                filter,
            } = self;
            seq.add_node(parent_path, filter)
        }
    }
    impl<'a, F> Runnable<F> for AddTerminalNode<'a, F> {
        fn run(self, seq) -> Result<NodeIdStr, Error> {
            let Self {
                parent_path,
                filter,
            } = self;
            seq.add_terminal_node(parent_path, filter)
        }
    }
    impl<'a, F> Runnable<F> for SetNodeFilter<'a, F> {
        fn run(self, seq) -> Result<F, Error> {
            let Self {
                path,
                filter,
            } = self;
            seq.set_node_filter(path, filter)
        }
    }
    impl<'a, F> Runnable<F> for SetNodeItemWeight<'a> {
        fn run(self, seq) -> Result<Weight, Error> {
            let Self {
                path,
                item_index,
                weight,
            } = self;
            seq.set_node_item_weight(path, item_index, weight)
        }
    }
    impl<'a, F> Runnable<F> for SetNodeWeight<'a> {
        fn run(self, seq) -> Result<Weight, Error> {
            let Self {
                path,
                weight,
            } = self;
            seq.set_node_weight(path, weight)
        }
    }
    impl<'a, F> Runnable<F> for SetNodeOrderType<'a> {
        fn run(self, seq) -> Result<OrderType, Error> {
            let Self {
                path,
                order_type,
            } = self;
            seq.set_node_order_type(path, order_type)
        }
    }
    impl<'a, F> Runnable<F> for UpdateNodes<'a> {
        fn run(self, seq) -> Result<Success, Error> {
            let Self {
                path,
            } = self;
            seq.update_nodes(path)
        }
    }
    impl<'a, F> Runnable<F> for RemoveNode<'a> {
        fn run(self, seq) -> Result<Success, Error> {
            let Self {
                id
            } = self;
            seq.remove_node(id).map(|_| ())
        }
    }
}
impl<T: ItemSource<F>, F> Sequencer<T, F>
where
    F: Clone,
{
    /// Runs the specified [`Command`] and returns the result
    ///
    /// # Errors
    /// Returns an [`Error`] when inputs do not match the inner tree state
    pub fn run<U>(&mut self, command: U) -> Result<U::Output, Error>
    where
        U: Runnable<F>,
    {
        command.run(self)
    }
}
