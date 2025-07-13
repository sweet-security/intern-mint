use std::io::Write;

use databuf::{Decode, Encode};

use crate::interned::Interned;

impl Encode for Interned {
    #[inline]
    fn encode<const CONFIG: u16>(&self, w: &mut (impl Write + ?Sized)) -> std::io::Result<()> {
        Encode::encode::<CONFIG>(&self, w)
    }
}

impl Decode<'_> for Interned {
    #[inline]
    fn decode<const CONFIG: u16>(c: &mut &[u8]) -> databuf::Result<Self> {
        let data = Vec::<u8>::decode::<CONFIG>(c)?;
        Ok(data.into())
    }
}
