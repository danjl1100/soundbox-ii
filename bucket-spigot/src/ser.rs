// Copyright (C) 2021-2025  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

//! Serialize/deserialize a [`Network`] via a sequence of [`ModifyCmdRef`]s

use crate::{
    order::OrderType, path::Path, traversal::TraversalElem, ModifyCmd, ModifyCmdRef, ModifyError,
    Network,
};

/// Visitor for [`ModifyCmdRef`] elements to serialize a [`Network`]
///
/// Inspired by and blanket implemented for [`serde::ser::SerializeSeq`]
///
/// For simplicity, see [`VecVisitor`]
pub(crate) trait Visitor<T, U> {
    type Ok;
    type Error;

    /// Visit a command.
    ///
    /// The implementor is responsible for cloning the [`ModifyCmdRef`] if needed for the usecase
    ///
    /// # Errors
    /// Returns an error if the serialization fails
    fn visit(&mut self, cmd: ModifyCmdRef<'_, T, U>) -> Result<(), Self::Error>;

    /// Finish the visitor and return the result
    ///
    /// # Errors
    /// Returns an error if the finalization fails
    fn finish(self) -> Result<Self::Ok, Self::Error>;
}

mod vec_visitor {
    use super::Visitor;
    use crate::ModifyCmdRef;

    pub(super) enum Never {}

    /// Simple [`Visitor`] that clones the items into a [`Vec`]
    pub(super) struct VecVisitor<T, U, V, F>
    where
        F: FnMut(ModifyCmdRef<'_, T, U>) -> V,
    {
        elems: Vec<V>,
        map_fn: F,
        _marker: std::marker::PhantomData<(T, U)>,
    }

    impl<T, U, V, F> VecVisitor<T, U, V, F>
    where
        F: FnMut(ModifyCmdRef<'_, T, U>) -> V,
    {
        pub(super) fn new(map_fn: F) -> Self {
            Self {
                elems: vec![],
                map_fn,
                _marker: std::marker::PhantomData,
            }
        }
    }
    impl<T, U, V, F> Visitor<T, U> for VecVisitor<T, U, V, F>
    where
        F: FnMut(ModifyCmdRef<'_, T, U>) -> V,
    {
        type Ok = Vec<V>;
        type Error = Never;
        fn visit(&mut self, cmd: ModifyCmdRef<'_, T, U>) -> Result<(), Never> {
            let Self { elems, map_fn, .. } = self;
            elems.push(map_fn(cmd));
            Ok(())
        }
        fn finish(self) -> Result<Vec<V>, Never> {
            let Self { elems, .. } = self;
            Ok(elems)
        }
    }

    impl<T, U, V, F> std::fmt::Debug for VecVisitor<T, U, V, F>
    where
        V: std::fmt::Debug,
        F: std::fmt::Debug,
        F: FnMut(ModifyCmdRef<'_, T, U>) -> V,
    {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("VecVisitor")
                .field("elems", &self.elems)
                .field("map_fn", &self.map_fn)
                .finish()
        }
    }
    impl<T, U, V, F> Clone for VecVisitor<T, U, V, F>
    where
        V: Clone,
        F: Clone,
        F: FnMut(ModifyCmdRef<'_, T, U>) -> V,
    {
        fn clone(&self) -> Self {
            let Self {
                elems,
                map_fn,
                _marker,
            } = self;
            Self {
                elems: elems.clone(),
                map_fn: map_fn.clone(),
                _marker: std::marker::PhantomData,
            }
        }
    }
}

impl<T, U, V, O, E> Visitor<T, U> for V
where
    V: serde::ser::SerializeSeq<Ok = O, Error = E>,
    for<'a> ModifyCmdRef<'a, T, U>: serde::Serialize,
{
    type Ok = O;
    type Error = E;
    fn visit(&mut self, cmd: ModifyCmdRef<'_, T, U>) -> Result<(), E> {
        self.serialize_element(&cmd)
    }
    fn finish(self) -> Result<Self::Ok, Self::Error> {
        self.end()
    }
}

impl<T, U> Network<T, U>
where
    T: crate::clap::ArgBounds + serde::Serialize,
    U: crate::clap::ArgBounds,
{
    /// Serialize into a lines for use with [`crate::clap::ModifyCmd`]
    #[allow(unused)] // TODO for fn: as_command_lines
    #[must_use]
    pub(crate) fn serialize_as_command_lines(&self) -> Vec<String> {
        let visitor =
            vec_visitor::VecVisitor::new(|modify_cmd| modify_cmd.display_as_cmd().to_string());
        self.serialize(visitor)
            .unwrap_or_else(|never| match never {})
    }
}
impl<T, U> Network<T, U> {
    /// Serialize into a vector
    #[must_use]
    pub fn serialize_collect(&self) -> Vec<ModifyCmd<T, U>>
    where
        T: Clone,
        U: Clone,
    {
        // TODO minimze the example, why is the closure needed for type inference?
        #[expect(clippy::redundant_closure_for_method_calls)]
        let visitor = vec_visitor::VecVisitor::new(|modify_cmd_ref| modify_cmd_ref.to_owned());
        self.serialize(visitor)
            .unwrap_or_else(|never| match never {})
    }
    // TODO is there any use-case for serializing from a specific node? like, for (non-tabular) views?
    fn serialize<V>(&self, mut dest: V) -> Result<V::Ok, V::Error>
    where
        V: Visitor<T, U>,
    {
        {
            let root_order_type = self.trees.order.node().get_order_type();
            if root_order_type != OrderType::default() {
                dest.visit(ModifyCmdRef::SetOrderType {
                    path: Path::empty().as_ref(),
                    new_order_type: root_order_type,
                })?;
            }
        }

        self.trees.try_visit_depth_first(|elem| {
            let TraversalElem {
                node_path: path,
                parent_weights,
                node_weight,
                node_item,
                node_order,
            } = elem;

            let (_last, parent) = path.split_last().expect("node should not be pathless root");
            let creation_cmd = match node_item {
                crate::Child::Bucket(_) => ModifyCmdRef::AddBucket { parent },
                crate::Child::Joint(_) => ModifyCmdRef::AddJoint { parent },
            };
            dest.visit(creation_cmd)?;

            let order_type = node_order.get_order_type();
            if order_type != OrderType::default() {
                dest.visit(ModifyCmdRef::SetOrderType {
                    path,
                    new_order_type: order_type,
                })?;
            }

            if parent_weights.is_none_or(|w| !w.is_unity()) {
                dest.visit(ModifyCmdRef::SetWeight {
                    path,
                    new_weight: node_weight,
                })?;
            }

            // TODO
            // let filters = node_item.get_filters();
            // if !filters.is_empty() {
            //     dest.visit(ModifyCmdRef::SetFilters {
            //         path,
            //         new_filters: filters,
            //     })?;
            // }

            // if let crate::Child::Bucket(bucket) = &node_item {
            //     let items = &bucket.items;
            //     if !items.is_empty() {
            //         dest.visit(ModifyCmdRef::FillBucket {
            //             bucket: path,
            //             new_contents: items,
            //         })?;
            //     }
            // }

            Ok(())
        })?;

        dest.finish()
    }
}

impl<T, U> Network<T, U>
where
    T: serde::Serialize,
    U: serde::Serialize,
{
    /// Serialize as a sequence of [`ModifyCmdRef`]s
    ///
    /// Avoids the (small) overhead of collecting into a [`Vec`] before serializing
    ///
    /// # Errors
    /// Forwards serializer errors
    ///
    /// # Example
    /// ```
    /// use bucket_spigot::Network;
    /// #[derive(serde::Serialize)]
    /// struct Stored {
    ///     #[serde(serialize_with = "Network::serialize_into_modify_commands")]
    ///     network: Network<String, String>,
    /// }
    /// ```
    pub fn serialize_into_modify_commands<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let seq = serializer.serialize_seq(None)?;
        self.serialize(seq)
    }
}

impl<T, U> Network<T, U>
where
    T: serde::de::DeserializeOwned,
    U: serde::de::DeserializeOwned,
{
    /// Deserialize from a sequence of [`ModifyCmd`]s
    ///
    /// # Errors
    /// Forwards deserializer errors, or any [`ModifyError`]s
    ///
    /// # Example
    /// ```
    /// use bucket_spigot::Network;
    /// #[derive(serde::Deserialize)]
    /// struct Stored {
    ///     #[serde(deserialize_with = "Network::deserialize_from_modify_commands")]
    ///     network: Network<String, String>,
    /// }
    /// ```
    pub fn deserialize_from_modify_commands<'de, D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Visitor<T, U>(Network<T, U>);
        impl<'a, T, U> serde::de::Visitor<'a> for Visitor<T, U>
        where
            T: serde::de::DeserializeOwned,
            U: serde::de::DeserializeOwned,
        {
            type Value = Network<T, U>;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "sequence of ModifyCmd")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'a>,
            {
                use serde::de::Error;
                let Self(mut network) = self;
                while let Some(cmd) = seq.next_element()? {
                    network.modify(cmd).map_err(A::Error::custom)?;
                }
                Ok(network)
            }
        }
        deserializer.deserialize_seq(Visitor(Network::default()))
    }
}

// Deserialize is FromIterator
impl<T, U> FromIterator<ModifyCmd<T, U>> for Result<Network<T, U>, ModifyError> {
    fn from_iter<I: IntoIterator<Item = ModifyCmd<T, U>>>(cmds: I) -> Self {
        let mut network = Network::default();
        for cmd in cmds {
            network.modify(cmd)?;
        }
        Ok(network)
    }
}

#[cfg(test)]
mod proof_serde_integration {
    #![allow(dead_code)]

    use crate::Network;

    #[derive(serde::Serialize)]
    struct ProofSerialize<T, U>
    where
        T: serde::Serialize,
        U: serde::Serialize,
    {
        #[serde(serialize_with = "Network::serialize_into_modify_commands")]
        network: Network<T, U>,
    }

    #[derive(serde::Deserialize)]
    struct ProofDeserialize<T, U>
    where
        T: serde::de::DeserializeOwned,
        U: serde::de::DeserializeOwned,
    {
        #[serde(deserialize_with = "Network::deserialize_from_modify_commands")]
        network: Network<T, U>,
    }

    #[derive(serde::Serialize, serde::Deserialize)]
    struct ProofSerializeDeserialize<T, U>
    where
        T: serde::Serialize + serde::de::DeserializeOwned,
        U: serde::Serialize + serde::de::DeserializeOwned,
    {
        #[serde(serialize_with = "Network::serialize_into_modify_commands")]
        #[serde(deserialize_with = "Network::deserialize_from_modify_commands")]
        network: Network<T, U>,
    }
}
