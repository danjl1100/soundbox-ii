// Copyright (C) 2021-2023  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

//! Converts from KDL types to a Rust type by driving a `serde::Deserializer`

use crate::persistence::KdlEntryVisitor;
use std::collections::VecDeque;

type SuperError = super::Error<shared::Never>;

// Stores the entities visited by [`KdlEntryVisitor`], in order to trigger equivalent callbacks in
// [`serde::de::Deserializer`]
#[derive(Default)]
pub struct DeserializeVisitor {
    entries: VecDeque<Entry>,
    current_key: Option<String>,
    current_value: Option<Value>,
}
#[derive(Debug, Clone, PartialEq)]
pub enum Entry {
    Property { key: String, value: Value },
}
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    String(String),
    I64(i64),
    Bool(bool),
}

#[derive(Debug, PartialEq)]
pub enum Error {
    UnimplementedType(&'static str),
    NextKeyExistingKey(String),
    NextKeyExistingValue(Value),
    ValueTypeMismatch {
        expected: &'static str,
        value: Value,
    },
    IntOutOfRange(i64),
    PendingEntries(Vec<Entry>),
    PendingKey(String),
    PendingValue(Value),
    MissingPreparedKey,
    MissingPreparedValue(&'static str),
    UnexpectedKdlArgument {
        value: Value,
        after_entry: Option<Entry>,
    },
}
impl Error {
    fn unimplemented_type(ty: &'static str) -> SuperError {
        SuperError::Deserialize(Error::UnimplementedType(ty))
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnimplementedType(ty) => {
                write!(f, "KDL to serde-deserialize unimplemented for type {ty:?}")
            }
            Self::NextKeyExistingKey(key) => write!(
                f,
                "when requesting next key, pending un-processed key {key:?}"
            ),
            Self::NextKeyExistingValue(value) => write!(
                f,
                "when requesting next key, pending un-processed value {value:?}"
            ),
            Self::ValueTypeMismatch { expected, value } => {
                write!(f, "expected {expected}, found: {value:?}")
            }
            Self::IntOutOfRange(int) => write!(f, "integer out of range: {int}"),
            Self::PendingEntries(entries) => {
                write!(f, "finished, but pending entries: {entries:?}")
            }
            Self::PendingKey(key) => write!(f, "finished, but pending key: {key:?}"),
            Self::PendingValue(value) => write!(f, "finished, but pending value: {value:?}"),
            Self::MissingPreparedKey => write!(f, "no key requested to be processed"),
            Self::MissingPreparedValue(ty) => {
                write!(f, "no value (ty {ty}) requested to be processed")
            }
            Self::UnexpectedKdlArgument { value, after_entry } => {
                write!(
                    f,
                    "expected key/value KDL pairs, found argument with no key: {value:?}"
                )?;
                if let Some(after_entry) = after_entry {
                    write!(f, " after entry {after_entry:?}")
                } else {
                    Ok(())
                }
            }
        }
    }
}
impl From<Error> for SuperError {
    fn from(err: Error) -> Self {
        SuperError::Deserialize(err)
    }
}

impl DeserializeVisitor {
    pub fn check_finish(self) -> Result<(), Error> {
        let Self {
            entries,
            current_key,
            current_value,
        } = self;
        if !entries.is_empty() {
            Err(Error::PendingEntries(entries.into()))
        } else if let Some(current_key) = current_key {
            Err(Error::PendingKey(current_key))
        } else if let Some(current_value) = current_value {
            Err(Error::PendingValue(current_value))
        } else {
            Ok(())
        }
    }
    fn parse_int<T>(&mut self) -> Result<T, Error>
    where
        T: TryFrom<i64>,
        T::Error: std::fmt::Display,
    {
        const TYPE: &str = "int";
        match self.current_value.take() {
            Some(Value::I64(value)) => value.try_into().map_err(|_| Error::IntOutOfRange(value)),
            Some(value) => Err(Error::ValueTypeMismatch {
                expected: TYPE,
                value,
            }),
            None => Err(Error::MissingPreparedValue(TYPE)),
        }
    }
    fn unexpected_kdl_argument(&self, value: Value) -> Error {
        let after_entry = self.entries.iter().last().cloned();
        Error::UnexpectedKdlArgument { value, after_entry }
    }
}
impl KdlEntryVisitor for DeserializeVisitor {
    type Error = SuperError;

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
        Err(self
            .unexpected_kdl_argument(Value::String(value.to_string()))
            .into())
    }
    fn visit_argument_i64(&mut self, value: i64) -> Result<(), Self::Error> {
        Err(self.unexpected_kdl_argument(Value::I64(value)).into())
    }
    fn visit_argument_bool(&mut self, value: bool) -> Result<(), Self::Error> {
        Err(self.unexpected_kdl_argument(Value::Bool(value)).into())
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
    type Error = SuperError;

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
            Err(Error::MissingPreparedKey.into())
        }
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        const TYPE: &str = "bool";
        let value = match self.current_value.take() {
            Some(Value::Bool(value)) => Ok(value),
            Some(value) => Err(Error::ValueTypeMismatch {
                expected: TYPE,
                value,
            }),
            None => Err(Error::MissingPreparedValue(TYPE)),
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
        Err(Error::unimplemented_type("f32"))
    }

    fn deserialize_f64<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        Err(Error::unimplemented_type("f64"))
    }

    fn deserialize_char<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        Err(Error::unimplemented_type("char"))
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        const TYPE: &str = "str";
        let value = match self.current_value.take() {
            Some(Value::String(value)) => Ok(value),
            Some(value) => Err(Error::ValueTypeMismatch {
                expected: TYPE,
                value,
            }),
            None => Err(Error::MissingPreparedValue(TYPE)),
        }?;
        visitor.visit_str(&value)
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        const TYPE: &str = "string";
        let value = match self.current_value.take() {
            Some(Value::String(value)) => Ok(value),
            Some(value) => Err(Error::ValueTypeMismatch {
                expected: TYPE,
                value,
            }),
            None => Err(Error::MissingPreparedValue(TYPE)),
        }?;
        visitor.visit_string(value)
    }

    fn deserialize_bytes<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        Err(Error::unimplemented_type("bytes"))
    }

    fn deserialize_byte_buf<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        Err(Error::unimplemented_type("byte_buf"))
    }

    fn deserialize_option<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        Err(Error::unimplemented_type("option"))
    }

    fn deserialize_unit<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        Err(Error::unimplemented_type("unit"))
    }

    fn deserialize_unit_struct<V>(
        self,
        _name: &'static str,
        _visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        Err(Error::unimplemented_type("unit_struct"))
    }

    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        _visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        Err(Error::unimplemented_type("newtype_struct"))
    }

    fn deserialize_seq<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        Err(Error::unimplemented_type("seq"))
    }

    fn deserialize_tuple<V>(self, _len: usize, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        Err(Error::unimplemented_type("tuple"))
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
        Err(Error::unimplemented_type("tuple_struct"))
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
        Err(Error::unimplemented_type("enum"))
    }

    fn deserialize_ignored_any<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        Err(Error::unimplemented_type("ignored_any"))
    }
}

impl<'de, 'a> serde::de::MapAccess<'de> for &'a mut DeserializeVisitor {
    type Error = SuperError;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: serde::de::DeserializeSeed<'de>,
    {
        match self.entries.pop_front() {
            Some(Entry::Property { key, value }) => {
                let existing_key = self.current_key.replace(key);
                let existing_value = self.current_value.replace(value);

                if let Some(existing_key) = existing_key {
                    Err(Error::NextKeyExistingKey(existing_key).into())
                } else if let Some(existing_value) = existing_value {
                    Err(Error::NextKeyExistingValue(existing_value).into())
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
