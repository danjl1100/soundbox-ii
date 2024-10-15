// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

//! Serialize/deserialize a [`Network`] via a sequence of [`ModifyCmd`]s

use crate::{
    order::OrderType, path::Path, traversal::TraversalElem, ModifyCmd, ModifyError, Network,
};

/// Visitor for [`ModifyCmd`] elements to serialize a [`Network`]
///
/// Inspired by and blanket implemented for [`serde::ser::SerializeSeq`]
///
/// For simplicity, see [`VecVisitor`]
pub(crate) trait Visitor<T, U> {
    type Ok;
    type Error;

    /// Visit a command.
    ///
    /// The implementor is responsible for cloning the [`ModifyCmd`] if needed for the usecase
    ///
    /// # Errors
    /// Returns an error if the serialization fails
    fn visit(&mut self, cmd: &ModifyCmd<T, U>) -> Result<(), Self::Error>;

    /// Finish the visitor and return the result
    ///
    /// # Errors
    /// Returns an error if the finalization fails
    fn finish(self) -> Result<Self::Ok, Self::Error>;
}

mod vec_visitor {
    use super::Visitor;
    use crate::ModifyCmd;

    pub(super) enum Never {}

    /// Simple [`Visitor`] that clones the items into a [`Vec`]
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub(super) struct VecVisitor<T, U>(Vec<ModifyCmd<T, U>>);
    impl<T, U> Default for VecVisitor<T, U> {
        fn default() -> Self {
            Self(vec![])
        }
    }
    impl<T, U> Visitor<T, U> for VecVisitor<T, U>
    where
        T: Clone,
        U: Clone,
    {
        type Ok = Vec<ModifyCmd<T, U>>;
        type Error = Never;
        fn visit(&mut self, cmd: &ModifyCmd<T, U>) -> Result<(), Never> {
            self.0.push(cmd.clone());
            Ok(())
        }
        fn finish(self) -> Result<Vec<ModifyCmd<T, U>>, Never> {
            Ok(self.0)
        }
    }
}

impl<T, U, V, O, E> Visitor<T, U> for V
where
    V: serde::ser::SerializeSeq<Ok = O, Error = E>,
    ModifyCmd<T, U>: serde::Serialize,
{
    type Ok = O;
    type Error = E;
    fn visit(&mut self, cmd: &ModifyCmd<T, U>) -> Result<(), E> {
        self.serialize_element(cmd)
    }
    fn finish(self) -> Result<Self::Ok, Self::Error> {
        self.end()
    }
}

// TODO is this macro complexity worth it? (removes some `clones`). This may become more complex with `if`, etc.
// NOTE add optimizations **after** the test passes
macro_rules! reuse_path {
    ($dest:ident . $method:ident ( &ModifyCmd:: $variant:ident {
        $path:ident $(,)?
        $($field:ident : $value:expr),* $(,)?
    })?;) => {
        let cmd = ModifyCmd::$variant {
            $path,
            $($field : $value),*
        };
        $dest.$method(&cmd)?;
        let ModifyCmd::$variant { $path, .. } = cmd else {
            unreachable!()
        };
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
        self.serialize(vec_visitor::VecVisitor::default())
            .unwrap_or_else(|never| match never {})
    }
    // TODO is there any use-case for serializing from a specific node? like, for (non-tablurar) views?
    fn serialize<V>(&self, mut dest: V) -> Result<V::Ok, V::Error>
    where
        V: Visitor<T, U>,
    {
        {
            let root_order_type = self.trees.order.node().get_order_type();
            if root_order_type != OrderType::default() {
                dest.visit(&ModifyCmd::SetOrderType {
                    path: Path::empty(),
                    new_order_type: root_order_type,
                })?;
            }
        }

        self.trees.try_visit_depth_first(|elem| {
            let TraversalElem {
                node_path,
                parent_weights,
                node_weight,
                node_item,
                node_order,
            } = elem;

            // TODO with ModifyCmd accepting PathRef, this would be a lot simpler
            //      (e.g. remove the move dances below, which are there to only clone once)
            //
            // let (_last, parent) = node_path
            //     .split_last()
            //     .expect("node should not be pathless root");
            // let parent = parent.to_owned();
            // let creation_cmd = match node_item {
            //     crate::Child::Bucket(_) => ModifyCmd::AddBucket { parent },
            //     crate::Child::Joint(_) => ModifyCmd::AddJoint { parent },
            // };
            // dest.visit(&creation_cmd)?;

            let path = {
                // split last, parent
                let (last, parent) = node_path
                    .split_last()
                    .expect("node should not be pathless root");
                let parent = parent.to_owned();
                let parent = match node_item {
                    crate::Child::Bucket(_) => {
                        reuse_path! {
                            dest.visit(&ModifyCmd::AddBucket { parent })?;
                        }
                        parent
                    }
                    crate::Child::Joint(_) => {
                        reuse_path! {
                            dest.visit(&ModifyCmd::AddJoint { parent })?;
                        }
                        parent
                    }
                };
                // joint last, parent
                let mut path = parent;
                path.push(last);
                path
            };

            let path = {
                let order_type = node_order.get_order_type();
                if order_type == OrderType::default() {
                    path
                } else {
                    reuse_path! {
                        dest.visit(&ModifyCmd::SetOrderType {
                            path,
                            new_order_type: order_type,
                        })?;
                    }
                    path
                }
            };

            let path = {
                if parent_weights.map_or(true, |w| !w.is_unity()) {
                    reuse_path! {
                        dest.visit(&ModifyCmd::SetWeight {
                            path,
                            new_weight: node_weight,
                        })?;
                    }
                    path
                } else {
                    path
                }
            };

            drop(path);

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
    /// Serialize as a sequence of [`ModifyCmd`]
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
    /// Deserialize from a sequence of [`ModifyCmd`]
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
