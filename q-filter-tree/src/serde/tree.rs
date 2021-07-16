use crate::{id::NodePath, node::NodeInfo, Tree};

use core::marker::PhantomData;
use serde::de::{Deserialize, Deserializer, Error, MapAccess, Visitor};
use serde::ser::{Serialize, SerializeMap, Serializer};

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
            let node_path = NodePath::from(node_id);
            let node_info = NodeInfo::from(node);
            map.serialize_entry(&node_path, &node_info)?;
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
        let mut tree = Tree::new();
        let mut weights = std::collections::HashMap::new();

        while let Some((node_path, node_info)) = access.next_entry::<NodePath, NodeInfo<T, F>>()? {
            // get NodeId (ROOT, or INSERT)
            let node_id = if let Some((parent_path, _)) = node_path.parent() {
                // create node
                tree.add_child(&parent_path, None).map_err(|_| {
                    M::Error::custom(format!(
                        "failed to create node at path {}, parent {:?} does not exist",
                        node_path, parent_path
                    ))
                })?
            } else {
                // root node already exists
                tree.root_id()
            };

            // get Node
            let node = tree.get_node_mut(&node_id).map_err(|_| {
                M::Error::custom(format!("newly-created node_id is invalid: {:?}", node_id))
            })?;

            let (info_intrinsic, child_weights) = node_info.into();
            // update Node
            node.overwrite_from(info_intrinsic);

            // record weights
            weights.insert(node_path, child_weights);
        }

        // set child_weights (finishing pass)
        for (node_path, child_weights) in weights {
            let node = tree.get_node_mut(&node_path).map_err(|_| {
                M::Error::custom(format!(
                    "failed to locate node {} during finishing pass",
                    node_path
                ))
            })?;
            node.overwrite_child_weights(child_weights)
                .map_err(|(weights, orig_len)| {
                    M::Error::custom(format!(
                        "failed to set node {} child weights (weights len = {}, but child nodes len = {})",
                        node_path, weights.len(), orig_len,
                    ))
                })?;
        }

        Ok(tree)
    }
}
