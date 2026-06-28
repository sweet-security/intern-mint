use std::{
    cmp::Ordering,
    hash::{Hash, Hasher},
    ops::Deref,
};

use crate::{
    interned::{self, Interned},
    pool::POOL,
};

#[derive(Eq)]
#[repr(transparent)]
/// &[BorrowedInterned] exists to pass around instead of cloning [Interned] instances when not
/// needed, and in order to avoid passing &[Interned] which will require double-dereference to
/// access the data
///
/// # Example
///
/// &[BorrowedInterned] can be used with hash-maps
///
/// Note that the pointer is being used for hashing and comparing
/// (see [Hash] and [PartialEq] trait implementations)
///
/// As opposed to hashing and comparing the actual data - because the pointers are
/// unique for the same data as long as it "lives" in memory
///
/// ```
/// use intern_mint::{BorrowedInterned, Interned};
///
/// let map = std::collections::HashMap::<Interned, u64>::from_iter([(Interned::new(b"key"), 1)]);
///
/// let key = Interned::new(b"key");
/// assert_eq!(map.get(&key), Some(&1));
///
/// let borrowed_key: &BorrowedInterned = &key;
/// assert_eq!(map.get(borrowed_key), Some(&1));
/// ```
/// &[BorrowedInterned] can be used with btree-maps
///
/// ```
/// use intern_mint::{BorrowedInterned, Interned};
///
/// let map = std::collections::BTreeMap::<Interned, u64>::from_iter([(Interned::new(b"key"), 1)]);
///
/// let key = Interned::new(b"key");
/// assert_eq!(map.get(&key), Some(&1));
///
/// let borrowed_key: &BorrowedInterned = &key;
/// assert_eq!(map.get(borrowed_key), Some(&1));
/// ```
pub struct BorrowedInterned([u8]);

impl BorrowedInterned {
    pub(crate) fn new(value: &[u8]) -> &BorrowedInterned {
        unsafe { &*(value as *const [u8] as *const BorrowedInterned) }
    }

    /// Constructs back an [Interned] value from the given `&BorrowedInterned`.
    ///
    /// # Panics
    ///
    /// Panics if the underlying pool entry is no longer present. In safe code this
    /// cannot happen because a `&BorrowedInterned` can only be obtained from a live
    /// [Interned] (which keeps the entry alive). If you need a non-panicking variant,
    /// use [`try_intern`][BorrowedInterned::try_intern].
    ///
    /// Note that using this function has almost the same performance penalty as using
    /// [Interned::new].
    #[track_caller]
    pub fn intern(&self) -> Interned {
        self.try_intern()
            .expect("pool entry is missing; the BorrowedInterned must outlive its Interned owner")
    }

    /// Attempts to construct an [Interned] value from the given `&BorrowedInterned`,
    /// returning `None` if the pool entry is no longer present.
    ///
    /// In safe code this always returns `Some`, because a `&BorrowedInterned` can only
    /// be obtained from a live [Interned] (which keeps the pool entry alive). This
    /// method is provided as a non-panicking alternative to [`intern`][BorrowedInterned::intern]
    /// for callers who need to handle the absence case gracefully.
    pub fn try_intern(&self) -> Option<Interned> {
        POOL.get_from_existing_ref(self.deref())
            .map(Interned::from_existing)
    }

    /// The default [Hash] trait implementation for [BorrowedInterned] is to hash the pointer
    /// instead of the data (for performance gains)
    ///
    /// This is allowed because the pointers are unique for the same data as long as it "lives" in
    /// memory
    ///
    /// If for some reason you need the hash of the actual data, this function can be used
    ///
    /// # Example
    ///
    /// ```
    /// use std::hash::{BuildHasher, Hasher};
    ///
    /// use intern_mint::Interned;
    ///
    /// let hash_builder = ahash::RandomState::new();
    ///
    /// let hash_data = |data: &Interned| {
    ///     let mut hasher = hash_builder.build_hasher();
    ///     data.hash_data(&mut hasher);
    ///     hasher.finish()
    /// };
    ///
    /// let (ptr_hash_1, data_hash_1) = {
    ///     let interned = Interned::new(b"hello!");
    ///     (hash_builder.hash_one(&interned), hash_data(&interned))
    /// };
    ///
    /// let (ptr_hash_2, data_hash_2) = {
    ///     let _a = Interned::new(b"more allocations");
    ///     let _a = Interned::new(b"to avoid");
    ///     let _a = Interned::new(b"same address");
    ///
    ///     let interned = Interned::new(b"hello!");
    ///     (hash_builder.hash_one(&interned), hash_data(&interned))
    /// };
    ///
    /// // The hash of the pointers is different, but the hash of the data is the same
    /// assert_ne!(ptr_hash_1, ptr_hash_2);
    /// assert_eq!(data_hash_1, data_hash_2);
    /// ```
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

    /// Converts `&BorrowedInterned` back into an owned [Interned].
    ///
    /// Delegates to [`intern`][BorrowedInterned::intern]; see also the non-panicking
    /// [`try_intern`][BorrowedInterned::try_intern].
    fn to_owned(&self) -> Self::Owned {
        self.intern()
    }
}
