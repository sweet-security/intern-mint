use std::fmt::{self, Formatter};

use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{SeqAccess, Visitor},
};

use crate::interned::Interned;

impl Serialize for Interned {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let bytes: &[u8] = self;
        // For human-readable formats (e.g. JSON) prefer a string so the output is compact and
        // interoperable. Fall back to bytes for non-UTF-8 data so the round-trip stays lossless,
        // and always use bytes for non-human-readable (binary) formats.
        if serializer.is_human_readable() {
            match std::str::from_utf8(bytes) {
                Ok(s) => serializer.serialize_str(s),
                Err(_) => serializer.serialize_bytes(bytes),
            }
        } else {
            serializer.serialize_bytes(bytes)
        }
    }
}

impl<'de> Deserialize<'de> for Interned {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(InternedVisitor)
    }
}

struct InternedVisitor;

impl<'de> Visitor<'de> for InternedVisitor {
    type Value = Interned;

    fn expecting(&self, formatter: &mut Formatter) -> fmt::Result {
        formatter.write_str("a string, byte array, or sequence of bytes")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Interned::new(v.as_bytes()))
    }

    fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Interned::new(v.as_bytes()))
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Interned::new(v))
    }

    fn visit_byte_buf<E>(self, v: Vec<u8>) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Interned::new(&v))
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut bytes = Vec::with_capacity(seq.size_hint().unwrap_or(0));
        while let Some(byte) = seq.next_element::<u8>()? {
            bytes.push(byte);
        }
        Ok(Interned::new(&bytes))
    }
}
