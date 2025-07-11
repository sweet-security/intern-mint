use std::{
    borrow::Borrow,
    cmp::Ordering,
    ffi::{OsStr, OsString},
    hash::{Hash, Hasher},
    ops::Deref,
    path::{Path, PathBuf},
};

use triomphe::Arc;

use crate::{borrow::BorrowedInterned, pool::POOL};

#[derive(Clone, Eq)]
#[repr(transparent)]
pub struct Interned(Arc<[u8]>);

impl Interned {
    pub fn new(value: &[u8]) -> Self {
        Self(POOL.get_or_insert(value))
    }

    pub fn hash_data<H: Hasher>(&self, state: &mut H) {
        self.deref().hash(state);
        0u8.hash(state);
    }
}

impl Drop for Interned {
    fn drop(&mut self) {
        POOL.remove_if_needed(&self.0);
    }
}

impl Deref for Interned {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

impl PartialEq for Interned {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::addr_eq(self.as_ptr(), other.as_ptr())
    }
}

impl Hash for Interned {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_ptr().hash(state);
    }
}

impl PartialOrd for Interned {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Interned {
    fn cmp(&self, other: &Self) -> Ordering {
        self.deref().cmp(other.deref())
    }
}

impl From<&[u8]> for Interned {
    fn from(value: &[u8]) -> Self {
        Interned::new(value)
    }
}
impl From<Vec<u8>> for Interned {
    fn from(value: Vec<u8>) -> Self {
        value.as_slice().into()
    }
}

impl From<&str> for Interned {
    fn from(value: &str) -> Self {
        value.as_bytes().into()
    }
}

impl From<String> for Interned {
    fn from(value: String) -> Self {
        value.as_bytes().into()
    }
}

impl From<&OsStr> for Interned {
    fn from(value: &OsStr) -> Self {
        value.as_encoded_bytes().into()
    }
}

impl From<OsString> for Interned {
    fn from(value: OsString) -> Self {
        value.as_encoded_bytes().into()
    }
}

impl From<&Path> for Interned {
    fn from(value: &Path) -> Self {
        value.as_os_str().into()
    }
}

impl From<PathBuf> for Interned {
    fn from(value: PathBuf) -> Self {
        value.as_os_str().into()
    }
}

impl Borrow<BorrowedInterned> for Interned {
    fn borrow(&self) -> &BorrowedInterned {
        BorrowedInterned::new(self.deref())
    }
}

impl AsRef<BorrowedInterned> for Interned {
    fn as_ref(&self) -> &BorrowedInterned {
        BorrowedInterned::new(self.deref())
    }
}
