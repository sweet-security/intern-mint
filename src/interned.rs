use std::{
    borrow::Borrow,
    cmp::Ordering,
    ffi::{OsStr, OsString},
    hash::{Hash, Hasher},
    ops::Deref,
    path::{Path, PathBuf},
    sync::LazyLock,
};

use triomphe::Arc;

use crate::{borrow::BorrowedInterned, pool::POOL};

#[derive(Clone, Eq)]
#[repr(transparent)]
/// The main type offered by this crate, responsible for interning slices
pub struct Interned(Arc<[u8]>);

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
        Self(POOL.get_or_insert(value))
    }

    pub(crate) fn from_existing(value: Arc<[u8]>) -> Self {
        Self(value)
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
        POOL.remove_if_needed(&self.0);
    }
}

impl Deref for Interned {
    type Target = BorrowedInterned;

    fn deref(&self) -> &Self::Target {
        BorrowedInterned::new(self.0.deref())
    }
}

impl PartialEq for Interned {
    /// Compares by **pointer identity** rather than data content (delegates to
    /// [`BorrowedInterned::eq`]).
    ///
    /// This is sound and consistent with [`Ord`] because the global intern pool upholds a
    /// deduplication invariant: for any two live [`Interned`] values, equal data implies equal
    /// pointers (the pool returns the same [`triomphe::Arc`] allocation for the same byte
    /// sequence). Consequently:
    ///
    /// - `ptr_a == ptr_b` ⟺ `data_a == data_b` (for live entries)
    /// - `cmp(a, b) == Equal` ⟺ `data_a == data_b` ⟺ `ptr_a == ptr_b` ⟺ `eq(a, b)`
    ///
    /// The [`Ord`] / [`PartialOrd`] impls compare by data; equality under [`Ord`] therefore
    /// coincides with pointer equality under [`PartialEq`], so the `Ord`/`Eq` contract
    /// (`a == b` ↔ `a.cmp(b) == Ordering::Equal`) holds.
    fn eq(&self, other: &Self) -> bool {
        self.deref().eq(other)
    }
}

impl Hash for Interned {
    /// Hashes by **pointer address** rather than data content, consistent with the pointer-based
    /// [`PartialEq`] impl (delegates to [`BorrowedInterned::hash`]).
    ///
    /// See [`PartialEq`] for why pointer equality is equivalent to data equality for live interned
    /// values (the pool deduplication invariant).
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
    /// Compares by **data content** (lexicographic byte order, delegates to
    /// [`BorrowedInterned::cmp`]), consistent with [`PartialEq`].
    ///
    /// Although [`PartialEq`] uses pointer identity and [`Ord`] uses data, the two are consistent
    /// because the pool's deduplication invariant guarantees that same-data ⟺ same-pointer for
    /// any two live values. Therefore `cmp(a, b) == Ordering::Equal` if and only if
    /// `a.as_ptr() == b.as_ptr()`, satisfying the requirement that `a == b` ↔
    /// `a.cmp(b) == Ordering::Equal`.
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
