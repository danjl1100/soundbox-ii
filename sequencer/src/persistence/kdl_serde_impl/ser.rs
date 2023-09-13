// Copyright (C) 2021-2023  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use super::{never::NeverSerialize, Error};
use crate::persistence::KdlEntryVisitor;
use serde::Serialize;

pub struct Serializer<V> {
    visitor: V,
    pending_key: Option<&'static str>,
}
impl<V> Serializer<V>
where
    V: KdlEntryVisitor,
{
    pub fn new(visitor: V) -> Self {
        Self {
            visitor,
            pending_key: None,
        }
    }
    pub fn finish(self) -> Result<V, Error<V::Error>> {
        let Self {
            visitor,
            pending_key,
        } = self;
        if let Some(pending_key) = pending_key {
            Err(Error::Message(format!("pending key {pending_key:?}")))
        } else {
            Ok(visitor)
        }
    }
}
impl<'a, V> serde::ser::Serializer for &'a mut Serializer<V>
where
    V: KdlEntryVisitor,
{
    type Ok = ();
    type Error = Error<V::Error>;

    // No additional state needed
    type SerializeSeq = NeverSerialize<V>;
    type SerializeTuple = NeverSerialize<V>;
    type SerializeTupleStruct = NeverSerialize<V>;
    type SerializeTupleVariant = NeverSerialize<V>;
    type SerializeMap = NeverSerialize<V>;
    type SerializeStruct = Self;
    type SerializeStructVariant = NeverSerialize<V>;

    fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error> {
        if let Some(pending_key) = self.pending_key.take() {
            self.visitor.visit_property_bool(pending_key, v)
        } else {
            self.visitor.visit_argument_bool(v)
        }
        .map_err(Error::KdlEntryVisitor)
    }

    fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(i64::from(v))
    }

    fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(i64::from(v))
    }

    fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(i64::from(v))
    }

    fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error> {
        if let Some(pending_key) = self.pending_key.take() {
            self.visitor.visit_property_i64(pending_key, v)
        } else {
            self.visitor.visit_argument_i64(v)
        }
        .map_err(Error::KdlEntryVisitor)
    }

    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(i64::from(v))
    }

    fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(i64::from(v))
    }

    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(i64::from(v))
    }

    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
        if let Ok(v) = i64::try_from(v) {
            self.serialize_i64(v)
        } else {
            Err(Error::Message(format!("u64 exceeds i64 range: {v}")))
        }
    }

    fn serialize_f32(self, _v: f32) -> Result<Self::Ok, Self::Error> {
        Err(Error::Message("unexpected float".to_string()))
    }

    fn serialize_f64(self, _v: f64) -> Result<Self::Ok, Self::Error> {
        Err(Error::Message("unexpected float".to_string()))
    }

    fn serialize_char(self, _v: char) -> Result<Self::Ok, Self::Error> {
        Err(Error::Message("unexpected char".to_string()))
    }

    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        if let Some(pending_key) = self.pending_key.take() {
            self.visitor.visit_property_str(pending_key, v)
        } else {
            self.visitor.visit_argument_str(v)
        }
        .map_err(Error::KdlEntryVisitor)
    }

    fn serialize_bytes(self, _v: &[u8]) -> Result<Self::Ok, Self::Error> {
        Err(Error::Message("unexpected bytes".to_string()))
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Err(Error::Message("unexpected none".to_string()))
    }

    fn serialize_some<T: ?Sized>(self, _value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize,
    {
        Err(Error::Message("unexpected some".to_string()))
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Err(Error::Message("unexpected unit".to_string()))
    }

    fn serialize_unit_struct(self, name: &'static str) -> Result<Self::Ok, Self::Error> {
        Err(Error::Message(format!("unexpected unit struct {name:?}")))
    }

    fn serialize_unit_variant(
        self,
        name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        Err(Error::Message(format!("unexpected unit variant {name:?}")))
    }

    fn serialize_newtype_struct<T: ?Sized>(
        self,
        name: &'static str,
        _value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize,
    {
        Err(Error::Message(format!(
            "unexpected newtype struct {name:?}"
        )))
    }

    fn serialize_newtype_variant<T: ?Sized>(
        self,
        name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize,
    {
        Err(Error::Message(format!(
            "unexpected newtype variant {name:?}"
        )))
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Err(Error::Message("unexpected seq".to_string()))
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Err(Error::Message("unexpected tuple".to_string()))
    }

    fn serialize_tuple_struct(
        self,
        name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Err(Error::Message(format!("unexpected tuple variant {name:?}")))
    }

    fn serialize_tuple_variant(
        self,
        name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(Error::Message(format!("unexpected tuple variant {name:?}")))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Err(Error::Message("unexpected map".to_string()))
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Ok(self)
    }

    fn serialize_struct_variant(
        self,
        name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(Error::Message(format!(
            "unexpected struct variant {name:?}"
        )))
    }
}
impl<'a, V> serde::ser::SerializeStruct for &'a mut Serializer<V>
where
    V: KdlEntryVisitor,
{
    type Ok = ();
    type Error = Error<V::Error>;

    fn serialize_field<T: ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        if let Some(existing_key) = self.pending_key.replace(key) {
            Err(Error::Message(format!(
                "serializing field {key:?} found unhandled key: {existing_key:?}"
            )))
        } else {
            value.serialize(&mut **self)
        }
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        if let Some(pending_key) = self.pending_key {
            Err(Error::Message(format!(
                "pending key while ending SerializeStruct: {pending_key:?}"
            )))
        } else {
            Ok(())
        }
    }
}
