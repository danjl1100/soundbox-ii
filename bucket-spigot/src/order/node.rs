// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

//! Tree structure for [`Order`], meant to mirror the
//! [`Network`](`crate::Network`) topology.

use super::{Order, OrderType};
use crate::path::{Path, PathRef};
use std::rc::Rc;

#[derive(Clone, Default, Debug)]
pub(crate) struct Root(pub(super) Node);
#[derive(Clone, Debug, Default)]
pub struct Node {
    pub(super) order: Order,
    pub(super) children: Vec<Rc<Node>>,
}

impl Root {
    /// Adds a default node at the specified path.
    ///
    /// Returns the index of the new child on success.
    pub(crate) fn add(&mut self, path: PathRef<'_>) -> Result<usize, UnknownOrderPath> {
        let parent = self.0.make_mut(path)?;
        let dest_children = &mut parent.children;

        let new_index = dest_children.len();

        dest_children.push(Rc::new(Node::default()));

        Ok(new_index)
    }
    pub(crate) fn set_order_type(
        &mut self,
        new_order_type: OrderType,
        path: PathRef<'_>,
    ) -> Result<(), UnknownOrderPath> {
        let dest = self.0.make_mut(path)?;

        if dest.order.get_ty() != new_order_type {
            dest.order = Order::new(new_order_type);
        }

        Ok(())
    }
    pub(crate) fn node(&self) -> &Node {
        &self.0
    }
}
impl Node {
    pub(crate) fn get_order_type(&self) -> OrderType {
        self.order.get_ty()
    }
    pub(crate) fn get_children(&self) -> &[Rc<Node>] {
        &self.children
    }
    fn make_mut(&mut self, path: PathRef<'_>) -> Result<&mut Self, UnknownOrderPath> {
        let mut current = self;

        for next_index in path {
            let Some(next) = current.children.get_mut(next_index) else {
                return Err(UnknownOrderPath(path.clone_inner()));
            };
            current = Rc::make_mut(next);
        }

        Ok(current)
    }
}

/// The specified path does not match an order-node
#[derive(Debug)]
pub struct UnknownOrderPath(pub(crate) Path);

impl std::fmt::Display for UnknownOrderPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self(path) = self;
        write!(f, "unknown order path: {path:?}")
    }
}
