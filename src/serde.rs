use bstr::BString;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::interned::Interned;

impl Serialize for Interned {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.as_bstr().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Interned {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        BString::deserialize(deserializer).map(|o| o.into())
    }
}
