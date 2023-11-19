// Copyright (C) 2021-2023  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

//! Converts from a Rust type to KDL by acting as a `serde::Serializer`

use super::never::NeverSerialize;
use crate::persistence::KdlEntryVisitor;
use serde::Serialize;

type SuperError<E> = super::Error<E>;

#[derive(Debug, PartialEq)]
pub enum Error {
    UnimplementedType {
        ty: &'static str,
        name: &'static str,
        pending_key: Option<&'static str>,
    },
    IntOutOfRange(u64),
    PendingKey {
        pending_key: &'static str,
        next_key: Option<&'static str>,
    },
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        const UNIMPLEMENTED_FOR: &str = "serde-serialize to KDL is unimplemented for";
        match self {
            Self::UnimplementedType {
                ty,
                name,
                pending_key,
            } => {
                write!(f, "{UNIMPLEMENTED_FOR} {ty} {name:?}")?;
                if let Some(key) = pending_key {
                    write!(f, " (key {key:?})")
                } else {
                    Ok(())
                }
            }
            Self::IntOutOfRange(int) => write!(f, "integer out of range of i64: {int}"),
            Self::PendingKey {
                pending_key,
                next_key: None,
            } => write!(f, "finished, but pending key: {pending_key:?}"),
            Self::PendingKey {
                pending_key,
                next_key: Some(next_key),
            } => write!(
                f,
                "encountered key {next_key:?} while still pending key: {pending_key:?}"
            ),
        }
    }
}
impl<E> From<Error> for SuperError<E> {
    fn from(err: Error) -> Self {
        SuperError::Serialize(err)
    }
}

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
    pub fn finish(self) -> Result<V, SuperError<V::Error>> {
        let Self {
            visitor,
            pending_key,
        } = self;
        if let Some(pending_key) = pending_key {
            Err(Error::PendingKey {
                pending_key,
                next_key: None,
            }
            .into())
        } else {
            Ok(visitor)
        }
    }

    fn unimplemented_type<E>(&self, name: &'static str) -> SuperError<E> {
        self.unimplemented("type", name)
    }

    fn unimplemented<E>(&self, ty: &'static str, name: &'static str) -> SuperError<E> {
        SuperError::Serialize(Error::UnimplementedType {
            ty,
            name,
            pending_key: self.pending_key,
        })
    }
}
impl<'a, V> serde::ser::Serializer for &'a mut Serializer<V>
where
    V: KdlEntryVisitor,
{
    type Ok = ();
    type Error = SuperError<V::Error>;

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
        .map_err(SuperError::KdlEntryVisitor)
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
        .map_err(SuperError::KdlEntryVisitor)
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
            Err(Error::IntOutOfRange(v).into())
        }
    }

    fn serialize_f32(self, _v: f32) -> Result<Self::Ok, Self::Error> {
        Err(self.unimplemented_type("float"))
    }

    fn serialize_f64(self, _v: f64) -> Result<Self::Ok, Self::Error> {
        Err(self.unimplemented_type("float"))
    }

    fn serialize_char(self, _v: char) -> Result<Self::Ok, Self::Error> {
        Err(self.unimplemented_type("char"))
    }

    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        if let Some(pending_key) = self.pending_key.take() {
            self.visitor.visit_property_str(pending_key, v)
        } else {
            self.visitor.visit_argument_str(v)
        }
        .map_err(SuperError::KdlEntryVisitor)
    }

    fn serialize_bytes(self, _v: &[u8]) -> Result<Self::Ok, Self::Error> {
        Err(self.unimplemented_type("bytes"))
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Err(self.unimplemented_type("none"))
    }

    fn serialize_some<T: ?Sized>(self, _value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize,
    {
        Err(self.unimplemented_type("some"))
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Err(self.unimplemented_type("unit"))
    }

    fn serialize_unit_struct(self, name: &'static str) -> Result<Self::Ok, Self::Error> {
        Err(self.unimplemented("unit struct", name))
    }

    fn serialize_unit_variant(
        self,
        name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        Err(self.unimplemented("unit variant", name))
    }

    fn serialize_newtype_struct<T: ?Sized>(
        self,
        name: &'static str,
        _value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize,
    {
        Err(self.unimplemented("newtype struct", name))
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
        Err(self.unimplemented("newtype variant", name))
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Err(self.unimplemented_type("seq"))
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Err(self.unimplemented_type("tuple"))
    }

    fn serialize_tuple_struct(
        self,
        name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Err(self.unimplemented("tuple struct", name))
    }

    fn serialize_tuple_variant(
        self,
        name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(self.unimplemented("tuple variant", name))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Err(self.unimplemented_type("map"))
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
        Err(self.unimplemented("struct variant", name))
    }
}
impl<'a, V> serde::ser::SerializeStruct for &'a mut Serializer<V>
where
    V: KdlEntryVisitor,
{
    type Ok = ();
    type Error = SuperError<V::Error>;

    fn serialize_field<T: ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        if let Some(pending_key) = self.pending_key.replace(key) {
            Err(Error::PendingKey {
                pending_key,
                next_key: Some(key),
            }
            .into())
        } else {
            value.serialize(&mut **self)
        }
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        if let Some(pending_key) = self.pending_key {
            Err(Error::PendingKey {
                pending_key,
                next_key: None,
            }
            .into())
        } else {
            Ok(())
        }
    }
}
