use std::{
    borrow::Borrow,
    cmp::Ordering,
    ffi::{OsStr, OsString},
    hash::{Hash, Hasher},
    mem::ManuallyDrop,
    ops::Deref,
    path::{Path, PathBuf},
    sync::LazyLock,
};

use triomphe::Arc;

use crate::{borrow::BorrowedInterned, pool::POOL};

#[derive(Clone, Eq)]
/// The main type offered by this crate, responsible for interning slices
///
/// The inner [Arc] is wrapped in [ManuallyDrop] so that, on drop, ownership of the [Arc] can be
/// handed to the pool ([`POOL::remove_on_last_drop`]). This lets the pool perform the final
/// reference-count decrement *while holding the shard lock*, which serializes it against the clones
/// done in `get_or_insert` / `get_from_existing_ref` and makes the last-drop eviction race-free.
/// [ManuallyDrop] is `repr(transparent)` over its inner `T`, so the in-memory layout is identical
/// to a bare `Arc<[u8]>` (nothing relies on [Interned]'s own `repr` beyond that; only
/// [`BorrowedInterned`]'s transparency is load-bearing).
pub struct Interned(ManuallyDrop<Arc<[u8]>>);

impl Interned {
    /// Constructs a new [Interned] for a given `value`
    ///
    /// # Example
    ///
    /// ```
    /// use intern_mint::Interned;
    ///
    /// let a = Interned::new(b"hello");
    /// let b = Interned::new(b"hello");
    ///
    /// assert_eq!(a.as_ptr(), b.as_ptr());
    /// ```
    pub fn new(value: &[u8]) -> Self {
        Self::from_existing(POOL.get_or_insert(value))
    }

    pub(crate) fn from_existing(value: Arc<[u8]>) -> Self {
        Self(ManuallyDrop::new(value))
    }
}

pub(crate) static DEFAULT: LazyLock<Interned> = LazyLock::new(|| Interned::new(Default::default()));

impl Default for Interned {
    fn default() -> Self {
        DEFAULT.clone()
    }
}

impl Drop for Interned {
    fn drop(&mut self) {
        // SAFETY: `self.0` is a live `ManuallyDrop` that is never taken anywhere else, and `self`
        // is being dropped, so this runs exactly once per handle and `self.0` is not used again
        // afterwards. Taking the `Arc` out hands ownership to the pool, which performs the final
        // reference-count decrement under the shard lock (see `remove_on_last_drop`).
        let arc = unsafe { ManuallyDrop::take(&mut self.0) };
        POOL.remove_on_last_drop(arc);
    }
}

impl Deref for Interned {
    type Target = BorrowedInterned;

    fn deref(&self) -> &Self::Target {
        // `self.0` is `ManuallyDrop<Arc<[u8]>>`; deref through both layers to reach the `[u8]`
        BorrowedInterned::new(&self.0)
    }
}

impl PartialEq for Interned {
    fn eq(&self, other: &Self) -> bool {
        self.deref().eq(other)
    }
}

impl Hash for Interned {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.deref().hash(state)
    }
}

impl PartialOrd for Interned {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Interned {
    fn cmp(&self, other: &Self) -> Ordering {
        self.deref().cmp(other)
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

impl From<&String> for Interned {
    fn from(value: &String) -> Self {
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

impl From<&OsString> for Interned {
    fn from(value: &OsString) -> Self {
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

impl From<&PathBuf> for Interned {
    fn from(value: &PathBuf) -> Self {
        value.as_os_str().into()
    }
}

impl Borrow<BorrowedInterned> for Interned {
    fn borrow(&self) -> &BorrowedInterned {
        self.deref()
    }
}

impl AsRef<BorrowedInterned> for Interned {
    fn as_ref(&self) -> &BorrowedInterned {
        self.deref()
    }
}
