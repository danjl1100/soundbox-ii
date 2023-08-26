// Copyright (C) 2021-2023  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use q_filter_tree::id::NodePathTyped;

use crate::{sources::ItemSource, Error, Sequencer};

impl<T, F> Sequencer<T, F>
where
    T: ItemSource<F>,
{
    /// Iterates over the specified node and its ancestors,
    /// calling the specified function for each filter element
    ///
    /// # Errors
    /// Returns an [`Error`] when inputs do not match the inner tree state
    pub fn with_ancestor_filters<U>(&self, path_str: &str, act_fn: &mut U) -> Result<(), Error>
    where
        U: FnMut(&NodePathTyped, &F),
    {
        let mut path = super::parse_path(path_str)?;
        loop {
            let (_, node) = path.try_ref_shared(&self.inner.tree)?;
            act_fn(&path, &node.filter);
            if let NodePathTyped::Child(child_path) = path {
                let (child_path, _) = child_path.into_parent();
                path = child_path;
            } else {
                break;
            }
        }
        Ok(())
    }
}
