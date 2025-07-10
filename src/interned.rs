use std::{
    borrow::Borrow,
    cmp::Ordering,
    hash::{Hash, Hasher},
    ops::Deref,
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
        std::ptr::eq(self.as_ptr(), other.as_ptr())
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
