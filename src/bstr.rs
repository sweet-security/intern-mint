use std::{
    borrow::Cow,
    ffi::OsStr,
    fmt::{Debug, Display, Formatter},
    ops::Deref,
    path::Path,
};

use bstr::{BStr, BString, ByteSlice};

use crate::{borrow::BorrowedInterned, interned::Interned};

impl BorrowedInterned {
    pub fn as_bstr(&self) -> &BStr {
        BStr::new(self.deref())
    }

    pub fn as_path(&self) -> Cow<'_, Path> {
        Cow::Borrowed(Path::new(self.as_os_str_ref()))
    }

    pub fn as_os_str(&self) -> Cow<'_, OsStr> {
        Cow::Borrowed(self.as_os_str_ref())
    }

    fn as_os_str_ref(&self) -> &OsStr {
        // SAFETY: `self.deref()` returns the exact bytes that were stored when this value was
        // interned, unchanged. When the value was interned from an `OsStr`/`OsString`/`Path`/
        // `PathBuf` (`Interned`'s `From<&OsStr>`/`From<&OsString>`/`From<&Path>`/... impls in
        // `interned.rs`), those bytes were produced by `OsStr::as_encoded_bytes()` on this same
        // platform and Rust build, which is exactly the documented precondition of
        // `OsStr::from_encoded_bytes_unchecked` (a self-contained slice from `as_encoded_bytes()`,
        // not split across an encoded boundary). This reconstructs the original `OsStr` losslessly,
        // including non-UTF-8 contents, instead of the previous UTF-8-lossy conversion.
        unsafe { OsStr::from_encoded_bytes_unchecked(self.deref()) }
    }

    pub fn as_str(&self) -> Cow<'_, str> {
        self.as_bstr().to_str_lossy()
    }
}

impl Display for Interned {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        Display::fmt(self as &BorrowedInterned, f)
    }
}

impl Display for BorrowedInterned {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        Display::fmt(self.as_bstr(), f)
    }
}

impl Debug for Interned {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        Debug::fmt(self as &BorrowedInterned, f)
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
