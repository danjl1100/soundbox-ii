// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use std::collections::BTreeMap;

use crate::ConfigOut;

pub(crate) fn serialize_map_keys_as_json<S, T>(
    values: &BTreeMap<Vec<String>, T>,
    s: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
    T: serde::Serialize,
{
    use serde::ser::Error as _;
    use serde::ser::SerializeMap as _;

    let len = values.len();
    let mut map = s.serialize_map(Some(len))?;

    for (key_raw, value) in values {
        let key_json = serde_json::to_string(&key_raw).map_err(S::Error::custom)?;
        map.serialize_entry(&key_json, value)?;
    }

    map.end()
}

pub(crate) fn deserialize_map_keys_as_json<'de, D>(
    de: D,
) -> Result<BTreeMap<Vec<String>, ConfigOut>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{Error as _, MapAccess, Visitor};

    struct MapVisitor;

    impl<'de> Visitor<'de> for MapVisitor {
        type Value = BTreeMap<Vec<String>, ConfigOut>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a map with JSON-encoded array keys")
        }

        fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
        where
            A: MapAccess<'de>,
        {
            let mut result = BTreeMap::new();
            while let Some((key_str, value)) = map.next_entry::<String, ConfigOut>()? {
                let key: Vec<String> = serde_json::from_str(&key_str).map_err(A::Error::custom)?;
                result.insert(key, value);
            }
            Ok(result)
        }
    }

    de.deserialize_map(MapVisitor)
}
