use std::{
    borrow::Cow,
    ffi::OsStr,
    fmt::{Debug, Display, Formatter},
    ops::Deref,
    path::Path,
};

use bstr::{BStr, BString, ByteSlice};

use crate::{borrow::BorrowedInterned, interned::Interned};

impl Interned {
    pub fn as_bstr(&self) -> &BStr {
        BStr::new(self.deref())
    }

    pub fn as_path(&self) -> Cow<'_, Path> {
        self.as_bstr().to_path_lossy()
    }

    pub fn as_os_str(&self) -> Cow<'_, OsStr> {
        self.as_bstr().to_os_str_lossy()
    }

    pub fn as_str(&self) -> Cow<'_, str> {
        self.as_bstr().to_str_lossy()
    }
}

impl BorrowedInterned {
    pub fn as_bstr(&self) -> &BStr {
        BStr::new(self.deref())
    }

    pub fn as_path(&self) -> Cow<'_, Path> {
        self.as_bstr().to_path_lossy()
    }

    pub fn as_os_str(&self) -> Cow<'_, OsStr> {
        self.as_bstr().to_os_str_lossy()
    }

    pub fn as_str(&self) -> Cow<'_, str> {
        self.as_bstr().to_str_lossy()
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
        Self::new(value.as_ref())
    }
}

impl From<BString> for Interned {
    fn from(value: BString) -> Self {
        value.as_bstr().into()
    }
}
