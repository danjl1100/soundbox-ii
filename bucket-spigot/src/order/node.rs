// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

//! Tree structure for [`Order`], meant to mirror the
//! [`Network`](`crate::Network`) topology.

use super::Order;
use crate::path::{Path, PathRef};
use std::rc::Rc;

#[derive(Clone, Default, Debug)]
pub(crate) struct Root(pub(super) Node);
#[derive(Clone, Debug, Default)]
pub struct Node {
    pub(crate) order: Order,
    pub(crate) children: Vec<Rc<Node>>,
}

impl Root {
    /// Adds a default node at the specified path.
    ///
    /// Returns the index of the new child on success.
    pub(crate) fn add(&mut self, path: PathRef<'_>) -> Result<usize, UnknownOrderPath> {
        let mut current_children = &mut self.0.children;

        for next_index in path {
            let Some(next_child) = current_children.get_mut(next_index) else {
                return Err(UnknownOrderPath(path.clone_inner()));
            };
            current_children = &mut Rc::make_mut(next_child).children;
        }

        let new_index = current_children.len();

        current_children.push(Rc::new(Node::default()));

        Ok(new_index)
    }
}

/// The specified path does not match an order-node
#[derive(Debug)]
pub struct UnknownOrderPath(pub(crate) Path);
