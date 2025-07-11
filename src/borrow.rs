use std::{
    cmp::Ordering,
    hash::{Hash, Hasher},
    ops::Deref,
};

use crate::interned::{self, Interned};

#[derive(Eq)]
#[repr(transparent)]
pub struct BorrowedInterned([u8]);

impl BorrowedInterned {
    pub(crate) fn new(value: &[u8]) -> &BorrowedInterned {
        unsafe { &*(value as *const [u8] as *const BorrowedInterned) }
    }

    pub fn intern(&self) -> Interned {
        Interned::new(self.deref())
    }

    pub fn hash_data<H: Hasher>(&self, state: &mut H) {
        self.deref().hash(state);
        0u8.hash(state);
    }
}

impl Default for &BorrowedInterned {
    fn default() -> Self {
        interned::DEFAULT.deref().as_ref()
    }
}

impl Deref for BorrowedInterned {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl PartialEq for BorrowedInterned {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::addr_eq(self.as_ptr(), other.as_ptr())
    }
}

impl Hash for BorrowedInterned {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_ptr().hash(state);
    }
}

impl PartialOrd for BorrowedInterned {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for BorrowedInterned {
    fn cmp(&self, other: &Self) -> Ordering {
        self.deref().cmp(other.deref())
    }
}

impl ToOwned for BorrowedInterned {
    type Owned = Interned;

    fn to_owned(&self) -> Self::Owned {
        self.intern()
    }
}
