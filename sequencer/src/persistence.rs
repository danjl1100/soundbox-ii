// Copyright (C) 2021-2023  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Provides helpers for persisting the user-editable configuration for the filter/source nodes

pub use self::single_root::Error as SingleRootError;
use self::single_root::{KdlNodesBySequence, SingleRootKdlDocument};
use crate::SequencerTree;
pub use io::SequencerConfigFile;
use kdl::KdlError;
use q_filter_tree::Weight;
use shared::Never;

mod parse;
mod update;

pub mod io;

const NAME_ROOT: &str = "root";
const NAME_CHAIN: &str = "chain";
const NAME_LEAF: &str = "leaf";

const ATTRIBUTE_WEIGHT: &str = "weight";
const DEFAULT_WEIGHT: Weight = 1;

// /// Fallible creation from a slice of [`KdlEntry`]s
// trait FromKdlEntries: Sized + Clone {
//     /// Error if the conversion fails
//     type Error;
//     /// Attempts to create from a slice of [`KdlEntry`]s
//     fn try_from(entries: &[KdlEntry]) -> Result<Self, (Self::Error, Option<miette::SourceSpan>)>;
// }
/// Fallible creation via a [`KdlEntryVisitor`]
pub trait FromKdlEntries: Sized + Clone {
    /// Error for the visitor and final creation
    type Error;
    /// Visitor which accepts key/value pairs
    type Visitor: KdlEntryVisitor<Error = Self::Error> + Default;
    /// Attempts to construct the type from the visitor information
    ///
    /// # Errors
    /// Returns an error if the visitor is not in a valid finished state
    fn try_finish(visitor: Self::Visitor) -> Result<Self, Self::Error>;
}
/// Fallible serialization via a [`KdlEntryVisitor`]
pub trait IntoKdlEntries: Sized + Clone {
    /// Error if the conversion fails
    type Error<E>;
    /// Informs the specified visitor of all key/value pairs required to reconstruct this type
    ///
    /// # Errors
    /// Returns an error if the conversion fails, possibly including a [`KdlEntryVisitor`] error
    fn try_into_kdl<V: KdlEntryVisitor>(&self, visitor: V) -> Result<V, Self::Error<V::Error>>;
}

/// Marker for external types that are implemented as a serde compatible map ([`String`] key-value pairs only)
pub trait StringMapSerializeDeserialize: serde::Serialize + serde::de::DeserializeOwned {}

/// Visitor capable of accepting [`kdl::KdlEntry`] types
#[allow(clippy::missing_errors_doc)]
pub trait KdlEntryVisitor {
    /// Error for serializing an entry
    type Error;

    /// Attempt to visit a key/value property of [`str`]
    fn visit_property_str(&mut self, key: &str, value: &str) -> Result<(), Self::Error>;
    /// Attempt to visit a key/value property of [`i64`]
    fn visit_property_i64(&mut self, key: &str, value: i64) -> Result<(), Self::Error>;
    /// Attempt to visit a key/value property of [`bool`]
    fn visit_property_bool(&mut self, key: &str, value: bool) -> Result<(), Self::Error>;

    /// Attempt to visit an argument of [`str`]
    fn visit_argument_str(&mut self, value: &str) -> Result<(), Self::Error>;
    /// Attempt to visit an argument of [`i64`]
    fn visit_argument_i64(&mut self, value: i64) -> Result<(), Self::Error>;
    /// Attempt to visit an argument of [`bool`]
    fn visit_argument_bool(&mut self, value: bool) -> Result<(), Self::Error>;
}

/// User-editable configuration for the filter/source nodes tree in a [`SequencerTree`]
///
/// This struct is used for saving the runtime state, in order to keep user-provided comments in
/// the original KDL input text.
pub struct SequencerConfig<T, F> {
    /// previous parsed/updated document, indexed by sequence
    previous_doc_nodes_flat: Option<KdlNodesBySequence>,
    _marker: std::marker::PhantomData<(T, F)>,
}
impl<T, F> Default for SequencerConfig<T, F>
where
    T: Clone,
    F: IntoKdlEntries,
{
    fn default() -> Self {
        Self {
            previous_doc_nodes_flat: None,
            _marker: std::marker::PhantomData,
        }
    }
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
        parse::parse_nodes(doc)
            .map(|(doc_flat, sequencer_tree)| {
                (
                    SequencerConfig {
                        previous_doc_nodes_flat: Some(doc_flat),
                        _marker: std::marker::PhantomData,
                    },
                    sequencer_tree,
                )
            })
            .map_err(ParseError::Node)
    }
}
impl<T, F> SequencerConfig<T, F>
where
    T: Clone,
    F: IntoKdlEntries,
{
    /// Updates the interal KDL document to match the specified [`SequencerTree`] and returns the
    /// KDL document text
    ///
    /// # Errors
    /// Returns an error if the filter serialization fails
    #[allow(clippy::missing_panics_doc)]
    pub fn update_to_string(
        &mut self,
        sequencer_tree: &SequencerTree<T, F>,
    ) -> Result<String, F::Error<Never>> {
        let result;
        self.previous_doc_nodes_flat = {
            let (new_doc_flat, result_inner) =
                update::update_for_nodes(self.previous_doc_nodes_flat.take(), sequencer_tree);
            result = result_inner;

            Some(new_doc_flat)
        };

        result
    }
}
// TODO deleteme, no longer a "cheap" operation, since it requires reinflating the flat_doc
// impl<T, F> SequencerConfig<T, F> {
//     /// Creates a non-annotated version of the internal [`KdlDocument`], from the last parse/update
//     pub(crate) fn calculate_nonannotated_doc(
//         &self,
//         seq_tree: &SequencerTree<T, F>,
//     ) -> Option<String> {
//         // TODO migrate to using `KdlNodesBySequence`
//         let mut doc = self.previous_annotated_doc.as_ref()?.clone();
//         annotate::strip_leading_seq(doc.single_root_mut());
//         Some(doc.to_string())
//     }
// }
impl<T, F> Clone for SequencerConfig<T, F> {
    fn clone(&self) -> Self {
        Self {
            previous_doc_nodes_flat: self.previous_doc_nodes_flat.clone(),
            _marker: self._marker,
        }
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

/// Error parsing [`SequencerTree`] nodes from the KDL input string
#[derive(Debug)]
#[non_exhaustive]
pub struct NodeError<E> {
    /// Location of the error within the KDL document
    pub span: miette::SourceSpan,
    /// Type of error
    pub kind: NodeErrorKind<E>,
}
/// Error kind for parsing [`SequencerTree`] nodes from the KDL input string
#[derive(Debug, PartialEq, Eq)]
pub enum NodeErrorKind<E> {
    /// Root node was not uniquely defined
    RootCount(SingleRootError),
    /// Invalid tag name on a node
    #[allow(missing_docs)]
    TagNameInvalid {
        found: String,
        expected: &'static [&'static str],
    },
    /// Tag requires a child block (even if empty `{}`)
    TagMissingChildBlock,
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
    /// Leaf node found with child nodes
    LeafNotEmpty,
}

#[cfg(test)]
#[allow(clippy::panic)] // tests are allowed to panic
mod tests {

    mod decode;

    mod encode;

    #[test]
    #[ignore]
    fn round_trip() {
        panic!("the discou")
    }
}

#[allow(clippy::module_name_repetitions)]
mod single_root {
    use kdl::{KdlDocument, KdlNode};
    use std::collections::HashMap;

    #[derive(Debug, Clone)]
    pub struct SingleRootKdlDocument(KdlDocument);
    impl SingleRootKdlDocument {
        // TODO delete unused
        // /// Returns a reference to the (known to be unique) root node
        // pub fn single_root(&self) -> &KdlNode {
        //     self.0.nodes().get(0).expect("single root")
        // }
        // /// Returns a mutable reference to the (known to be unique) root node
        // pub fn single_root_mut(&mut self) -> &mut KdlNode {
        //     self.0.nodes_mut().get_mut(0).expect("single root")
        // }
        /// Extract the document to perform top-level operations
        pub fn into_parts_remove_root(self) -> (EmptyKdlDocument, KdlNode) {
            let Self(mut inner) = self;
            let root = inner.nodes_mut().remove(0);
            let empty_doc = EmptyKdlDocument::try_from(inner)
                .expect("SingleRootKdlDocument with root removed is EmptyKdlDocument");
            (empty_doc, root)
        }
    }
    impl TryFrom<KdlDocument> for SingleRootKdlDocument {
        type Error = (Error, KdlDocument);
        fn try_from(doc: KdlDocument) -> Result<Self, Self::Error> {
            match doc.nodes().len() {
                1 => Ok(Self(doc)),
                0 => Err((Error::NoNodes, doc)),
                count => Err((Error::ManyNodes(count), doc)),
            }
        }
    }
    impl Default for SingleRootKdlDocument {
        fn default() -> Self {
            let mut doc = KdlDocument::new();
            doc.nodes_mut().push(KdlNode::new(super::NAME_ROOT));
            Self::try_from(doc).expect("added one and only one root")
        }
    }
    impl ToString for SingleRootKdlDocument {
        fn to_string(&self) -> String {
            self.0.to_string()
        }
    }

    /// Deconstructed [`KdlDocument`], with all nodes collected and indexed by the tree sequence
    #[derive(Clone, Debug, Default)]
    pub struct KdlNodesBySequence {
        seq_nodes: HashMap<u64, KdlNode>,
        // root_seq: Option<u64>,
        doc_empty: EmptyKdlDocument,
    }
    impl KdlNodesBySequence {
        /// Creates the collection with the specified document (only if it is empty)
        pub fn new(doc_empty: EmptyKdlDocument) -> Self {
            Self {
                seq_nodes: HashMap::new(),
                doc_empty,
            }
        }
        // TODO delete, no need to distinguish the root (just a specific sequence-referenced node)
        // pub fn insert_root(&mut self, root: KdlNode, seq: u64) -> (Option<u64>, Option<KdlNode>) {
        //     let prev_seq = self.root_seq.replace(seq);
        //     let prev_node = self.insert(root, seq);
        //     (prev_seq, prev_node)
        // }
        pub fn insert(&mut self, node: KdlNode, seq: u64) -> Option<KdlNode> {
            self.seq_nodes.insert(seq, node)
        }
        pub fn try_remove(&mut self, seq: u64) -> Option<KdlNode> {
            self.seq_nodes.remove(&seq)
        }
        pub fn end_delete_remaining(self) -> EmptyKdlDocument {
            let Self {
                seq_nodes: _,
                doc_empty,
            } = self;
            doc_empty
        }
        // TODO deleteme, after all usage the remaining nodes shall be deleted
        // pub fn try_end(self) -> Result<KdlDocument, Self> {
        //     let Self {
        //         seq_nodes,
        //         doc_empty,
        //     } = self;
        //     if seq_nodes.is_empty() {
        //         Ok(doc_empty)
        //     } else {
        //         Err(Self {
        //             seq_nodes,
        //             doc_empty,
        //         })
        //     }
        // }
    }

    /// [`KdlDocument`] known to have no child nodes
    #[derive(Clone, Debug, Default)]
    pub struct EmptyKdlDocument(KdlDocument);
    impl TryFrom<KdlDocument> for EmptyKdlDocument {
        type Error = KdlDocument;
        fn try_from(doc: KdlDocument) -> Result<Self, Self::Error> {
            if doc.nodes().is_empty() {
                Ok(Self(doc))
            } else {
                Err(doc)
            }
        }
    }
    impl EmptyKdlDocument {
        pub fn with_root(self, root: KdlNode) -> SingleRootKdlDocument {
            let Self(mut doc) = self;
            doc.nodes_mut().push(root);
            SingleRootKdlDocument::try_from(doc)
                .expect("EmptyKdlDocument plus single root is SingleRootKdlDocument")
        }
    }

    /// Invalid number of nodes
    #[allow(missing_docs)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Error {
        NoNodes,
        ManyNodes(usize),
    }
}
