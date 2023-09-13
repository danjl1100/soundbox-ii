// Copyright (C) 2021-2023  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

//! Bridges [`FromKdlEntries`]/[`IntoKdlEntries`] with [`serde::Serialize`] and
//! [`serde::de::DeserializeOwned`].
//!
//!
//! Note that the "serialization format" is the [`KdlEntries`] representation.
//!
//! 1. When a type `T` implements [`serde::de::DeserializeOwned`], then it may be created via [`FromKdlEntries`]
//!
//! 2. When a type `T` implements [`serde::Serialize`], then it may be stored [`IntoKdlEntries`]

use super::{FromKdlEntries, IntoKdlEntries, KdlEntryVisitor, StructSerializeDeserialize};

mod ser;

mod de;

mod never;

#[derive(Debug)]
pub enum Error<E> {
    Message(String),
    KdlEntryVisitor(E),
    // TODO add more like this:
    // ExpectedMap,
}

impl<E> std::error::Error for Error<E> where E: std::fmt::Debug {}
impl<E> std::fmt::Display for Error<E>
where
    E: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Message(message) => f.write_str(message),
            Error::KdlEntryVisitor(visitor_err) => write!(f, "visitor: {visitor_err:?}"),
            // TODO display example for the added error variants:
            // Error::ExpectedMap => f.write_str("expected map"),
        }
    }
}

impl<E> serde::ser::Error for Error<E>
where
    E: std::fmt::Debug,
{
    fn custom<T>(msg: T) -> Self
    where
        T: std::fmt::Display,
    {
        Error::Message(msg.to_string())
    }
}

impl<E> serde::de::Error for Error<E>
where
    E: std::fmt::Debug,
{
    fn custom<T>(msg: T) -> Self
    where
        T: std::fmt::Display,
    {
        Error::Message(msg.to_string())
    }
}

impl<T> IntoKdlEntries for T
where
    T: StructSerializeDeserialize,
{
    type Error<E> = Error<E>;

    fn try_into_kdl<V: KdlEntryVisitor>(&self, visitor: V) -> Result<V, Self::Error<V::Error>> {
        let mut serializer = ser::Serializer::new(visitor);
        self.serialize(&mut serializer)?;
        serializer.finish()
    }
}

impl<T> FromKdlEntries for T
where
    T: StructSerializeDeserialize,
{
    type Error = Error<shared::Never>;

    type Visitor = de::DeserializeVisitor;

    fn try_finish(mut de_visitor: Self::Visitor) -> Result<Self, Self::Error> {
        let value = T::deserialize(&mut de_visitor)?;
        de_visitor.check_finish()?;
        Ok(value)
    }
}
