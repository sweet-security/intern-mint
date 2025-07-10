use std::{
    fmt::{Debug, Display, Formatter},
    ops::Deref,
};

use bstr::{BStr, BString, ByteSlice};

use crate::{borrow::BorrowedInterned, interned::Interned};

impl Interned {
    pub fn as_bstr(&self) -> &BStr {
        BStr::new(self.deref())
    }
}

impl BorrowedInterned {
    pub fn as_bstr(&self) -> &BStr {
        BStr::new(self.deref())
    }
}

impl Display for Interned {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        Display::fmt(self.as_bstr(), f)
    }
}

impl Display for BorrowedInterned {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        Display::fmt(self.as_bstr(), f)
    }
}

impl Debug for Interned {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        Debug::fmt(self.as_bstr(), f)
    }
}

impl Debug for BorrowedInterned {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        Debug::fmt(self.as_bstr(), f)
    }
}

impl From<&BStr> for Interned {
    fn from(value: &BStr) -> Self {
        Interned::new(value.as_ref())
    }
}

impl From<BString> for Interned {
    fn from(value: BString) -> Self {
        value.as_bstr().into()
    }
}
