mod ser {
    use crate::{id, node::NodeInfo, Tree};

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
                let node_path = id::NodePath::from(node_id);
                let node_info = NodeInfo::from(node);
                map.serialize_entry(&node_path, &node_info)?;
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
}
mod de {
    use crate::{id, node::NodeInfo, Tree};

    use core::marker::PhantomData;
    use serde::de::{Deserialize, Deserializer, MapAccess, Visitor};

    impl<'de, T, F> Deserialize<'de> for Tree<T, F>
    where
        F: Deserialize<'de> + /* TODO: remove DEBUG */ std::fmt::Debug,
        T: Deserialize<'de> + /* TODO: remove DEBUG */ std::fmt::Debug,
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
        F: Deserialize<'de> + /* TODO: remove DEBUG */ std::fmt::Debug,
        T: Deserialize<'de> + /* TODO: remove DEBUG */ std::fmt::Debug,
    {
        type Value = Tree<T, F>;
        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("key value pairs (map) to construct Tree nodes")
        }
        fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
        where
            M: MapAccess<'de>,
        {
            // let mut tree = Tree::new();

            while let Some((node_path_str, node)) = access.next_entry::<String, NodeInfo<T, F>>()? {
                /* TODO: remove DEBUG */
                dbg!(node_path_str, node);
            }

            todo!()
            // Ok(tree)
        }
    }

    impl<'de> Deserialize<'de> for id::NodePath {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            todo!()
        }
    }
}
