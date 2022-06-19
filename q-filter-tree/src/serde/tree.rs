// Copyright (C) 2021-2022  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use crate::id::{ty, NodeIdTyped, NodePath, NodePathRefTyped, NodePathTyped};
use crate::node::meta::NodeInfo;
use crate::{Tree, Weight};

use core::marker::PhantomData;
use serde::de::{Deserialize, Deserializer, Error, MapAccess, Visitor};
use serde::ser::{Serialize, SerializeMap, Serializer};
use std::collections::HashMap;

impl<T, F> Serialize for Tree<T, F>
where
    F: Serialize + Clone,
    T: Serialize + Clone,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let node_count = self.sum_node_count();
        let mut map = serializer.serialize_map(Some(node_count))?;
        for (node_id, node) in self.enumerate() {
            let node_info = NodeInfo::from((*node).clone());
            map.serialize_entry(&node_id, &node_info)?;
        }
        map.end()
    }
}

impl<'de, T, F> Deserialize<'de> for Tree<T, F>
where
    F: Deserialize<'de>,
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(TreeVisitor {
            marker: PhantomData,
        })
    }
}
struct TreeVisitor<T, F> {
    marker: PhantomData<fn() -> Tree<T, F>>,
}
impl<'de, T, F> Visitor<'de> for TreeVisitor<T, F>
where
    F: Deserialize<'de>,
    T: Deserialize<'de>,
{
    type Value = Tree<T, F>;
    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("key value pairs (map) to construct Tree nodes")
    }
    fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
    where
        M: MapAccess<'de>,
    {
        let mut tree = None;
        let mut weights_root: Vec<Option<Weight>> = vec![];
        let mut weights_children: HashMap<NodePath<ty::Child>, _> = HashMap::new();

        while let Some((node_id, node_info)) = access.next_entry::<NodeIdTyped, NodeInfo<T, F>>()? {
            let node_path = NodePathTyped::from(node_id);
            let (child_weights, info_intrinsic) = node_info.into();

            // insert Node (if root, create tree)
            match (&node_path, &mut tree) {
                (NodePathTyped::Root(_), Some(_existing_tree)) => {
                    return Err(M::Error::custom("double-defined root"));
                }
                (NodePathTyped::Child(_), None) => {
                    return Err(M::Error::custom("child node defined before root"));
                }
                (NodePathTyped::Root(_), tree_opt) => {
                    let tree = Tree::new_with_root(info_intrinsic);
                    tree_opt.replace(tree);
                }
                (NodePathTyped::Child(node_path), Some(ref mut tree)) => {
                    let (parent_path, path_elem) = node_path.clone().into_parent();
                    // create node
                    let mut parent_ref = parent_path.try_ref(tree).map_err(|_| {
                        M::Error::custom(format!(
                            "failed to create node at path {}, parent {:?} does not exist",
                            NodePathRefTyped::from(node_path),
                            parent_path
                        ))
                    })?;
                    let weight_opts = match &parent_path {
                        NodePathTyped::Root(_) => &mut weights_root,
                        NodePathTyped::Child(parent) => {
                            weights_children.get_mut(parent).ok_or_else(|| {
                                M::Error::custom(format!(
                                        "parent path {:?} not found in weights_children (internal error?)",
                                        parent_path,
                                ))
                            })?
                        }
                    };
                    let weight = match weight_opts.get_mut(path_elem) {
                        Some(weight_opt) => weight_opt.take().ok_or_else(|| {
                            M::Error::custom(format!(
                                "duplicate use of weight for path {:?}, child index {}",
                                parent_path, path_elem,
                            ))
                        }),
                        None => Err(M::Error::custom(format!(
                            "path element out of bounds at {:?}, child index {}",
                            parent_path, path_elem
                        ))),
                    }?;

                    let parent_child_count = parent_ref.child_nodes_len();
                    if parent_child_count != path_elem {
                        return Err(M::Error::custom(format!(
                            "node declared out of order, parent has {} children, but desired destination {:?}",
                            parent_child_count, node_path,
                        )));
                    }
                    if let Some(mut parent_ref) = parent_ref.child_nodes() {
                        parent_ref.add_child_from(weight, Some(info_intrinsic));
                    } else {
                        return Err(M::Error::custom(format!(
                            "parent of node {:?} is a not chain type",
                            node_path
                        )));
                    }
                }
            }

            // record weights
            let some_weights = child_weights.into_iter().map(Option::Some).collect();
            match node_path {
                NodePathTyped::Child(node_path) => {
                    weights_children.insert(node_path, some_weights);
                }
                NodePathTyped::Root(_) => {
                    weights_root = some_weights;
                }
            }
        }
        tree.ok_or_else(|| M::Error::custom("no nodes defined"))
    }
}
