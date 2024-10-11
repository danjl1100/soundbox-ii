// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

//! Serialize/deserialize a [`Network`] via a sequence of [`ModifyCmd`]s

use crate::{order::OrderType, path::Path, ModifyCmd, ModifyError, Network};

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

// TODO is this macro complexity with it? (removes some `clones`). This may become more complex with `if`, etc.
// NOTE add optimizations **after** the test passes
// macro_rules! reuse_path {
//     ($dest:ident . $method:ident ( &ModifyCmd:: $variant:ident {
//         $path:ident ,
//         $($field:ident : $value:expr),* $(,)?
//     })?;) => {
//         let cmd = ModifyCmd::$variant {
//             path: $path,
//             $($field : $value),*
//         };
//         $dest.$method(&cmd)?;
//         let ModifyCmd::$variant { path: $path, .. } = cmd else {
//             unreachable!()
//         };
//     }
// }

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
    fn serialize<V>(&self, mut dest: V) -> Result<V::Ok, V::Error>
    where
        V: Visitor<T, U>,
    {
        let root_order_node = self.root_order.node();
        let mut path = Path::empty();
        {
            let root_order_type = root_order_node.get_order_type();
            if root_order_type != OrderType::default() {
                dest.visit(&ModifyCmd::SetOrderType {
                    path: path.clone(),
                    new_order_type: root_order_type,
                })?;
            }
        }

        // depth-first traversal
        let mut stack = {
            let child_items = &self.root;
            let child_order = root_order_node.get_children();
            vec![(0, child_items, child_order)]
        };
        while let Some(last) = stack.last() {
            let (index, child_items, child_order) = *last;
            let child_weights = child_items.weights();

            let node_item = child_items.children().get(index);
            let node_weight = child_weights.map_or(Some(0), |w| w.get(index));
            let node_order = child_order.get(index);
            let Some(((node_item, node_weight), node_order)) =
                node_item.zip(node_weight).zip(node_order)
            else {
                path.pop();
                stack.pop();
                continue;
            };
            let parent = path.clone();
            path.push(index);

            let creation_cmd = match node_item {
                crate::Child::Bucket(_) => ModifyCmd::AddBucket { parent },
                crate::Child::Joint(_) => ModifyCmd::AddJoint { parent },
            };
            dest.visit(&creation_cmd)?;

            {
                let order_type = node_order.get_order_type();
                if order_type != OrderType::default() {
                    dest.visit(&ModifyCmd::SetOrderType {
                        path: path.clone(),
                        new_order_type: order_type,
                    })?;
                }
            }

            {
                if child_weights.map_or(true, |w| !w.is_unity()) {
                    dest.visit(&ModifyCmd::SetWeight {
                        path: path.clone(),
                        new_weight: node_weight,
                    })?;
                }
            }

            // dbg!((index, &path, node_weight, node_order.get_order_type()));

            let inner_child = match (node_item, node_order.get_children()) {
                (crate::Child::Bucket(_), order_children) => {
                    debug_assert!(
                        order_children.is_empty(),
                        "bucket order-children should be empty"
                    );
                    None
                }
                (crate::Child::Joint(joint), order_children) => {
                    if joint.next.is_empty() {
                        None
                    } else {
                        Some((0, &joint.next, order_children))
                    }
                }
            };

            let last = stack
                .last_mut()
                .expect("last should be available within the loop");
            last.0 += 1;

            if let Some(inner_child) = inner_child {
                stack.push(inner_child);
            } else {
                path.pop();
            }
        }

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
