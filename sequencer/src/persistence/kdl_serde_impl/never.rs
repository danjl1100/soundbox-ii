// Copyright (C) 2021-2023  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use crate::persistence::KdlEntryVisitor;
use serde::Serialize;

/// Un-instantiable type that describes how it will never serialize types
#[allow(clippy::module_name_repetitions)]
pub enum NeverSerialize<V> {
    Never {
        never: shared::Never,
        _marker: std::marker::PhantomData<V>,
    },
}
impl<V> std::fmt::Debug for NeverSerialize<V> {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            NeverSerialize::Never { never, _marker } => match never {},
        }
    }
}
impl<V> std::fmt::Display for NeverSerialize<V> {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            NeverSerialize::Never { never, _marker } => match never {},
        }
    }
}
impl<V> serde::ser::SerializeSeq for NeverSerialize<V>
where
    V: KdlEntryVisitor,
{
    type Ok = ();
    type Error = super::Error<V::Error>;

    fn serialize_element<T: ?Sized>(&mut self, _value: &T) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        match *self {
            NeverSerialize::Never { never, _marker } => match never {},
        }
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        match self {
            NeverSerialize::Never { never, _marker } => match never {},
        }
    }
}
impl<V> serde::ser::SerializeTuple for NeverSerialize<V>
where
    V: KdlEntryVisitor,
{
    type Ok = ();
    type Error = super::Error<V::Error>;

    fn serialize_element<T: ?Sized>(&mut self, _value: &T) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        match self {
            NeverSerialize::Never { never, _marker } => match *never {},
        }
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        match self {
            NeverSerialize::Never { never, _marker } => match never {},
        }
    }
}
impl<V> serde::ser::SerializeTupleStruct for NeverSerialize<V>
where
    V: KdlEntryVisitor,
{
    type Ok = ();
    type Error = super::Error<V::Error>;

    fn serialize_field<T: ?Sized>(&mut self, _value: &T) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        match *self {
            NeverSerialize::Never { never, _marker } => match never {},
        }
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        match self {
            NeverSerialize::Never { never, _marker } => match never {},
        }
    }
}
impl<V> serde::ser::SerializeTupleVariant for NeverSerialize<V>
where
    V: KdlEntryVisitor,
{
    type Ok = ();
    type Error = super::Error<V::Error>;

    fn serialize_field<T: ?Sized>(&mut self, _value: &T) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        match *self {
            NeverSerialize::Never { never, _marker } => match never {},
        }
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        match self {
            NeverSerialize::Never { never, _marker } => match never {},
        }
    }
}
impl<V> serde::ser::SerializeMap for NeverSerialize<V>
where
    V: KdlEntryVisitor,
{
    type Ok = ();
    type Error = super::Error<V::Error>;

    fn serialize_key<T: ?Sized>(&mut self, _key: &T) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        match *self {
            NeverSerialize::Never { never, _marker } => match never {},
        }
    }

    fn serialize_value<T: ?Sized>(&mut self, _value: &T) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        match *self {
            NeverSerialize::Never { never, _marker } => match never {},
        }
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        match self {
            NeverSerialize::Never { never, _marker } => match never {},
        }
    }
}
impl<V> serde::ser::SerializeStruct for NeverSerialize<V>
where
    V: KdlEntryVisitor,
{
    type Ok = ();
    type Error = super::Error<V::Error>;

    fn serialize_field<T: ?Sized>(
        &mut self,
        _key: &'static str,
        _value: &T,
    ) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        match *self {
            NeverSerialize::Never { never, _marker } => match never {},
        }
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        match self {
            NeverSerialize::Never { never, _marker } => match never {},
        }
    }
}
impl<V> serde::ser::SerializeStructVariant for NeverSerialize<V>
where
    V: KdlEntryVisitor,
{
    type Ok = ();
    type Error = super::Error<V::Error>;

    fn serialize_field<T: ?Sized>(
        &mut self,
        _key: &'static str,
        _value: &T,
    ) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        match *self {
            NeverSerialize::Never { never, _marker } => match never {},
        }
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        match self {
            NeverSerialize::Never { never, _marker } => match never {},
        }
    }
}
