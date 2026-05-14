use std::{
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use serde::{
    Deserialize, Deserializer,
    de::{Error, IntoDeserializer, Visitor},
};

/// Wraps a field which could be filled with '~' (indicating a missing value)
///
/// None is returned if the field has the value '~' during deserializiation
/// Otherwise, Some(T) is returned.
pub struct CatchMissing<T>(Option<T>);

impl<T> DerefMut for CatchMissing<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T> Deref for CatchMissing<T> {
    type Target = Option<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'de, T> Deserialize<'de> for CatchMissing<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct MaybeVisitor<T>(PhantomData<T>);

        impl<'de, T> Visitor<'de> for MaybeVisitor<T>
        where
            T: Deserialize<'de>,
        {
            type Value = CatchMissing<T>;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a string or `~`")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: Error,
            {
                self.visit_string(value.to_owned())
            }

            fn visit_borrowed_str<E>(self, value: &'de str) -> Result<Self::Value, E>
            where
                E: Error,
            {
                if value == "~" {
                    Ok(CatchMissing(None))
                } else {
                    T::deserialize(value.into_deserializer()).map(|value| CatchMissing(Some(value)))
                }
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: Error,
            {
                if value == "~" {
                    Ok(CatchMissing(None))
                } else {
                    T::deserialize(value.into_deserializer()).map(|value| CatchMissing(Some(value)))
                }
            }
        }

        deserializer.deserialize_string(MaybeVisitor(PhantomData))
    }
}
