mod ser {
    use crate::{id, node, order, Tree};
    use serde::ser::{Serialize, SerializeMap, Serializer};
    impl<T, F> Serialize for Tree<T, F>
    where
        F: Serialize + Default,
        T: Serialize,
    {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let node_count = self.sum_node_count();
            let mut map = serializer.serialize_map(Some(node_count))?;
            for (node_id, node) in self.enumerate() {
                let node_path = id::NodePath::from(node_id);
                map.serialize_entry(&node_path, node)?;
            }
            map.end()
        }
    }
    impl Serialize for id::NodePath {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let path_str = self
                .elems()
                .iter()
                .map(|n| format!("{}", n))
                .collect::<Vec<_>>()
                .join(",");
            serializer.serialize_str(&path_str)
        }
    }
    impl<T, F> Serialize for node::WeightNodeVec<T, F>
    where
        F: Default,
    {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            self.weights().serialize(serializer)
        }
    }
    impl Serialize for order::State {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let ty = self.get_type();
            ty.serialize(serializer)
        }
    }
}
