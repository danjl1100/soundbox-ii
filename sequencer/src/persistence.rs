// Copyright (C) 2021-2023  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Provides helpers for persisting the user-editable configuration for the filter/source nodes

use crate::{Sequencer, SequencerTree};
use kdl::{KdlDocument, KdlError};
use q_filter_tree::Weight;

mod parse;

mod io;

// /// Fallible creation from a slice of [`KdlEntry`]s
// trait FromKdlEntries: Sized + Clone {
//     /// Error if the conversion fails
//     type Error;
//     /// Attempts to create from a slice of [`KdlEntry`]s
//     fn try_from(entries: &[KdlEntry]) -> Result<Self, (Self::Error, Option<miette::SourceSpan>)>;
// }
/// Fallible creation via a [`KdlEntryVistor`]
pub trait FromKdlEntries: Sized + Clone {
    /// Error for the visitor and final creation
    type Error;
    /// Visitor which accepts key/value pairs
    type Visitor: KdlEntryVistor<Error = Self::Error> + Default;
    /// Attempts to construct the type from the visitor information
    ///
    /// # Errors
    /// Returns an error if the visitor is not in a valid finished state
    fn try_finish(visitor: Self::Visitor) -> Result<Self, Self::Error>;
}
/// Fallible creation to a slice of [`KdlEntry`]s
trait IntoKdlEntries: Sized + Clone {
    /// Error if the conversion fails
    type Error<E>;
    /// Informs the specified visitor of all key/value pairs required to reconstruct this type
    fn try_into<V: KdlEntryVistor>(&self, visitor: V) -> Result<V, Self::Error<V::Error>>;
}

/// Marker for external types that are implemented as a serde compatible map ([`String`] key-value pairs only)
pub trait StringMapSerializeDeserialize: serde::Serialize + serde::de::DeserializeOwned {}

/// Visitor capable of accepting [`kdl::KdlEntry`] types
#[allow(clippy::missing_errors_doc)]
pub trait KdlEntryVistor {
    /// Error for serializing an entry
    type Error;

    /// Attempt to visit a key/value entry of [`str`]
    fn visit_entry_str(&mut self, key: &str, value: &str) -> Result<(), Self::Error>;
    /// Attempt to visit a key/value entry of [`i64`]
    fn visit_entry_i64(&mut self, key: &str, value: i64) -> Result<(), Self::Error>;
    /// Attempt to visit a key/value entry of [`bool`]
    fn visit_entry_bool(&mut self, key: &str, value: bool) -> Result<(), Self::Error>;

    /// Attempt to visit a value of [`str`]
    fn visit_value_str(&mut self, value: &str) -> Result<(), Self::Error>;
    /// Attempt to visit a value of [`i64`]
    fn visit_value_i64(&mut self, value: i64) -> Result<(), Self::Error>;
    /// Attempt to visit a value of [`bool`]
    fn visit_value_bool(&mut self, value: bool) -> Result<(), Self::Error>;
}

/// User-editable configuration for the filter/source nodes tree in a [`Sequencer`]
///
/// This struct is used for saving the runtime state, in order to keep user-provided comments in
/// the original KDL input text.
pub struct SequencerConfig<T, F> {
    previous_doc: KdlDocument,
    _marker: std::marker::PhantomData<(T, F)>,
}
impl<T, F> SequencerConfig<T, F>
where
    T: Clone,
    F: FromKdlEntries,
{
    /// Reads the config from a KDL string
    ///
    /// # Errors
    /// Returns an error if the string is not valid KDL for a [`SequencerTree`]
    pub fn parse_from_str(s: &str) -> Result<(Self, SequencerTree<T, F>), ParseError<F>> {
        let doc = s.parse().map_err(ParseError::KDL)?;
        parse::parse_nodes(&doc)
            .map(|sequencer_tree| {
                (
                    SequencerConfig {
                        previous_doc: doc,
                        _marker: std::marker::PhantomData,
                    },
                    sequencer_tree,
                )
            })
            .map_err(ParseError::Node)
    }

    /// Updates the interal KDL document to match the specified [`Sequencer`] and returns the
    /// KDL document text
    pub fn update_to_string(&mut self, sequencer: &Sequencer<T, F>) -> String
    where
        T: crate::ItemSource<F>,
    {
        update_for_nodes(&mut self.previous_doc, &sequencer.inner);
        self.previous_doc.to_string()
    }
}

/// Error parsing [`SequencerConfig`] from a string
#[derive(Debug)]
#[allow(missing_docs)]
#[non_exhaustive]
pub enum ParseError<F: FromKdlEntries> {
    KDL(KdlError),
    Node(NodeError<F::Error>),
}

/// Error parsing [`Sequencer`] nodes from the KDL input string
#[derive(Debug)]
#[non_exhaustive]
pub struct NodeError<E> {
    span: miette::SourceSpan,
    kind: NodeErrorKind<E>,
}
/// Error kind for parsing [`Sequencer`] nodes from the KDL input string
#[derive(Debug, PartialEq, Eq)]
pub enum NodeErrorKind<E> {
    /// Root node was not defined
    RootMissing,
    /// Invalid tag name on the root node
    #[allow(missing_docs)]
    RootTagNameInvalid {
        found: String,
        expected: &'static [&'static str],
    },
    /// Multiple nodes are defined as root
    RootDuplicate,
    /// Weight specified on root (this is not allowed)
    RootWeight,
    /// Node attributes failed to create valid filter
    AttributesInvalid(E),
    /// Node attribute type it not valid
    AttributeInvalidType,
    /// Weight type is not valid
    WeightInvalidType,
    /// Weight value is out of range
    WeightInvalidValue,
    /// Weight defined more than once for a node
    WeightDuplicate {
        /// Existing value
        first: (Weight, miette::SourceSpan),
        /// Duplicate value
        second: (Weight, miette::SourceSpan),
    },
}

fn update_for_nodes<T, F>(doc: &mut KdlDocument, sequencer: &SequencerTree<T, F>)
where
    T: Clone,
{
    todo!()
}

#[cfg(test)]
#[allow(clippy::panic)] // tests are allowed to panic
mod tests {

    mod decode;

    #[test]
    #[ignore]
    fn updates_and_stores_node_string() {
        panic!("the discoo");
    }

    #[test]
    #[ignore]
    fn round_trip() {
        panic!("the discou")
    }
}
