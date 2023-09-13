// Copyright (C) 2021-2023  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use crate::persistence::KdlEntryVisitor;
use std::collections::VecDeque;

type Error = super::Error<shared::Never>;

// Stores the entities visited by [`KdlEntryVisitor`], in order to trigger equivalent callbacks in
// [`serde::de::Deserializer`]
#[derive(Default)]
pub struct DeserializeVisitor {
    entries: VecDeque<Entry>,
    current_key: Option<String>,
    current_value: Option<Value>,
}
#[derive(Debug)]
enum Entry {
    Property { key: String, value: Value },
}
#[derive(Debug)]
enum Value {
    String(String),
    I64(i64),
    Bool(bool),
}

impl DeserializeVisitor {
    pub fn check_finish(self) -> Result<(), Error> {
        let Self {
            entries,
            current_key,
            current_value,
        } = self;
        if !entries.is_empty() {
            Err(Error::Message(format!(
                "entries not processed: {entries:?}"
            )))
        } else if let Some(current_key) = current_key {
            Err(Error::Message(format!(
                "key not processed: {current_key:?}"
            )))
        } else if let Some(current_value) = current_value {
            Err(Error::Message(format!(
                "value not processed: {current_value:?}"
            )))
        } else {
            Ok(())
        }
    }
    fn parse_int<T>(&mut self) -> Result<T, Error>
    where
        T: TryFrom<i64>,
        T::Error: std::fmt::Display,
    {
        match self.current_value.take() {
            Some(Value::I64(value)) => value
                .try_into()
                .map_err(|err| Error::Message(format!("int range error: {err:}"))),
            Some(value) => Err(Error::Message(format!("expected integer got {value:?}"))),
            None => Err(Error::Message(
                "serde expecting integer, but no value prepared".to_string(),
            )),
        }
    }
}
impl KdlEntryVisitor for DeserializeVisitor {
    type Error = Error;

    fn visit_property_str(&mut self, key: &str, value: &str) -> Result<(), Self::Error> {
        self.entries.push_back(Entry::Property {
            key: key.to_string(),
            value: Value::String(value.to_string()),
        });
        Ok(())
    }

    fn visit_property_i64(&mut self, key: &str, value: i64) -> Result<(), Self::Error> {
        self.entries.push_back(Entry::Property {
            key: key.to_string(),
            value: Value::I64(value),
        });
        Ok(())
    }

    fn visit_property_bool(&mut self, key: &str, value: bool) -> Result<(), Self::Error> {
        self.entries.push_back(Entry::Property {
            key: key.to_string(),
            value: Value::Bool(value),
        });
        Ok(())
    }

    fn visit_argument_str(&mut self, value: &str) -> Result<(), Self::Error> {
        Err(Error::Message(format!("unexpected argument {value:?}")))
    }
    fn visit_argument_i64(&mut self, value: i64) -> Result<(), Self::Error> {
        Err(Error::Message(format!("unexpected argument {value:?}")))
    }
    fn visit_argument_bool(&mut self, value: bool) -> Result<(), Self::Error> {
        Err(Error::Message(format!("unexpected argument {value:?}")))
    }

    // fn visit_argument_str(&mut self, value: &str) -> Result<(), Self::Error> {
    //     self.entries.push(Entry::Argument {
    //         value: Value::String(value.to_string()),
    //     });
    //     Ok(())
    // }

    // fn visit_argument_i64(&mut self, value: i64) -> Result<(), Self::Error> {
    //     self.entries.push(Entry::Argument {
    //         value: Value::I64(value),
    //     });
    //     Ok(())
    // }

    // fn visit_argument_bool(&mut self, value: bool) -> Result<(), Self::Error> {
    //     self.entries.push(Entry::Argument {
    //         value: Value::Bool(value),
    //     });
    //     Ok(())
    // }
}

impl<'de, 'a> serde::de::Deserializer<'de> for &'a mut DeserializeVisitor {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        self.deserialize_map(visitor)
    }
    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        // Err(Error::Message("cannot deserialize struct".to_string()))
        visitor.visit_map(self)
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_map(self)
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        // Err(Error::Message("cannot deserialize identifier".to_string()))
        if let Some(key) = self.current_key.take() {
            visitor.visit_string(key)
        } else {
            Err(Error::Message(
                "serde expecting identifier, but no key prepared".to_string(),
            ))
        }
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        let value = match self.current_value.take() {
            Some(Value::Bool(value)) => Ok(value),
            Some(value) => Err(Error::Message(format!("expected bool got {value:?}"))),
            None => Err(Error::Message(
                "serde expecting bool, but no value prepared".to_string(),
            )),
        }?;
        visitor.visit_bool(value)
    }

    fn deserialize_i8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_i8(self.parse_int()?)
    }

    fn deserialize_i16<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_i16(self.parse_int()?)
    }

    fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_i32(self.parse_int()?)
    }

    fn deserialize_i64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_i64(self.parse_int()?)
    }

    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_u8(self.parse_int()?)
    }

    fn deserialize_u16<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_u16(self.parse_int()?)
    }

    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_u32(self.parse_int()?)
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_u64(self.parse_int()?)
    }

    fn deserialize_f32<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        Err(Error::Message("cannot deserialize f32".to_string()))
    }

    fn deserialize_f64<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        Err(Error::Message("cannot deserialize f64".to_string()))
    }

    fn deserialize_char<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        Err(Error::Message("cannot deserialize char".to_string()))
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        let value = match self.current_value.take() {
            Some(Value::String(value)) => Ok(value),
            Some(value) => Err(Error::Message(format!("expected str got {value:?}"))),
            None => Err(Error::Message(
                "serde expecting str, but no value prepared".to_string(),
            )),
        }?;
        visitor.visit_str(&value)
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        let value = match self.current_value.take() {
            Some(Value::String(value)) => Ok(value),
            Some(value) => Err(Error::Message(format!("expected string got {value:?}"))),
            None => Err(Error::Message(
                "serde expecting string, but no value prepared".to_string(),
            )),
        }?;
        visitor.visit_string(value)
    }

    fn deserialize_bytes<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        Err(Error::Message("cannot deserialize bytes".to_string()))
    }

    fn deserialize_byte_buf<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        Err(Error::Message("cannot deserialize byte_buf".to_string()))
    }

    fn deserialize_option<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        Err(Error::Message("cannot deserialize option".to_string()))
    }

    fn deserialize_unit<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        Err(Error::Message("cannot deserialize unit".to_string()))
    }

    fn deserialize_unit_struct<V>(
        self,
        _name: &'static str,
        _visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        Err(Error::Message("cannot deserialize unit_struct".to_string()))
    }

    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        _visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        Err(Error::Message(
            "cannot deserialize newtype_struct".to_string(),
        ))
    }

    fn deserialize_seq<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        Err(Error::Message("cannot deserialize seq".to_string()))
    }

    fn deserialize_tuple<V>(self, _len: usize, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        Err(Error::Message("cannot deserialize tuple".to_string()))
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        _len: usize,
        _visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        Err(Error::Message(
            "cannot deserialize tuple_struct".to_string(),
        ))
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        _visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        Err(Error::Message("cannot deserialize enum".to_string()))
    }

    fn deserialize_ignored_any<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        Err(Error::Message("cannot deserialize ignored_any".to_string()))
    }
}

impl<'de, 'a> serde::de::MapAccess<'de> for &'a mut DeserializeVisitor {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: serde::de::DeserializeSeed<'de>,
    {
        match self.entries.pop_front() {
            Some(Entry::Property { key, value }) => {
                let existing_key = self.current_key.replace(key);
                let existing_value = self.current_value.replace(value);

                if let Some(existing_key) = existing_key {
                    Err(Error::Message(format!("existing key {existing_key:?}")))
                } else if let Some(existing_value) = existing_value {
                    Err(Error::Message(format!("existing value {existing_value:?}")))
                } else {
                    seed.deserialize(&mut **self).map(Some)
                }
            }
            None => Ok(None),
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::DeserializeSeed<'de>,
    {
        seed.deserialize(&mut **self)
    }
}
