use std::{
    cmp::Ordering,
    hash::{Hash, Hasher},
    ops::Deref,
};

#[derive(Eq)]
#[repr(transparent)]
pub struct BorrowedInterned([u8]);

impl BorrowedInterned {
    pub(crate) fn new(value: &[u8]) -> &BorrowedInterned {
        unsafe { &*(value as *const [u8] as *const BorrowedInterned) }
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
        std::ptr::eq(self.as_ptr(), other.as_ptr())
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
