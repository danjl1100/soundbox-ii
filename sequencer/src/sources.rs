// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! [`ItemSource`] types

pub use file::Lines as FileLines;
mod file;

pub use beet::Beet;
mod beet;

/// Source of items for the [`Sequencer`](`super::Sequencer`)
///
/// Generic `T` is the argument to the lookup, from each node in path to the terminal items node
pub trait ItemSource<T> {
    /// Element resulting from the lookup
    type Item: serde::Serialize + Clone + PartialEq;
    /// Error type if the lookup fails
    type Error: std::fmt::Display;
    /// Retrieves [`Item`](`Self::Item`)s matching the specified arguments (`T`)
    ///
    /// # Errors
    /// Returns an error if the underlying lookup operation fails
    fn lookup(&self, args: &[T]) -> Result<Vec<Self::Item>, Self::Error>;
}
