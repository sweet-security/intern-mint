use std::{
    borrow::Cow,
    ffi::OsStr,
    fmt::{Debug, Display, Formatter},
    ops::Deref,
    path::{Path, PathBuf},
};

use bstr::{BStr, BString, ByteSlice};

use crate::{borrow::BorrowedInterned, interned::Interned};

impl BorrowedInterned {
    pub fn as_bstr(&self) -> &BStr {
        BStr::new(self.deref())
    }

    /// Returns the interned bytes as a [`Path`].
    ///
    /// This is implemented in terms of [`as_os_str`](Self::as_os_str), so it inherits the same
    /// platform behavior: on unix it is a lossless, zero-copy borrow of the interned bytes; on
    /// non-unix platforms (Windows/WASI) it is a best-effort lossy conversion. See
    /// [`as_os_str`](Self::as_os_str) for details.
    pub fn as_path(&self) -> Cow<'_, Path> {
        match self.as_os_str() {
            Cow::Borrowed(os_str) => Cow::Borrowed(Path::new(os_str)),
            Cow::Owned(os_string) => Cow::Owned(PathBuf::from(os_string)),
        }
    }

    /// Returns the interned bytes as an [`OsStr`].
    ///
    /// On unix this is lossless and zero-copy: a unix [`OsStr`] is just bytes, so the interned
    /// bytes are reconstructed exactly (including non-UTF-8 contents) via
    /// [`OsStrExt::from_bytes`](std::os::unix::ffi::OsStrExt::from_bytes) and returned as a
    /// [`Cow::Borrowed`].
    ///
    /// On non-unix platforms (Windows/WASI) the result is a best-effort *lossy* conversion:
    /// invalid sequences are replaced with the U+FFFD replacement character. The standard library
    /// has no *safe* lossless reconstruction of an [`OsStr`] from arbitrary bytes on those
    /// platforms — the only lossless option, `OsStr::from_encoded_bytes_unchecked`, is `unsafe` and
    /// would be unsound here because an [`Interned`] may have been created from arbitrary bytes
    /// (e.g. `Interned::new(b"\xff\xff")`) that are not valid WTF-8.
    ///
    /// A lossless-on-Windows path could be offered later as an explicit `unsafe` opt-in method
    /// whose caller guarantees the interned bytes have [`OsStr`] provenance.
    #[cfg(unix)]
    pub fn as_os_str(&self) -> Cow<'_, OsStr> {
        use std::os::unix::ffi::OsStrExt;

        Cow::Borrowed(OsStr::from_bytes(self.deref()))
    }

    /// Returns the interned bytes as an [`OsStr`].
    ///
    /// On unix this is lossless and zero-copy: a unix [`OsStr`] is just bytes, so the interned
    /// bytes are reconstructed exactly (including non-UTF-8 contents) via
    /// [`OsStrExt::from_bytes`](std::os::unix::ffi::OsStrExt::from_bytes) and returned as a
    /// [`Cow::Borrowed`].
    ///
    /// On non-unix platforms (Windows/WASI) the result is a best-effort *lossy* conversion:
    /// invalid sequences are replaced with the U+FFFD replacement character. The standard library
    /// has no *safe* lossless reconstruction of an [`OsStr`] from arbitrary bytes on those
    /// platforms — the only lossless option, `OsStr::from_encoded_bytes_unchecked`, is `unsafe` and
    /// would be unsound here because an [`Interned`] may have been created from arbitrary bytes
    /// (e.g. `Interned::new(b"\xff\xff")`) that are not valid WTF-8.
    ///
    /// A lossless-on-Windows path could be offered later as an explicit `unsafe` opt-in method
    /// whose caller guarantees the interned bytes have [`OsStr`] provenance.
    #[cfg(not(unix))]
    pub fn as_os_str(&self) -> Cow<'_, OsStr> {
        self.as_bstr().to_os_str_lossy()
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
