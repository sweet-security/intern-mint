use std::sync::LazyLock;

use hashbrown::HashTable;
use parking_lot::{Mutex, MutexGuard};
use triomphe::ThinArc;

/// Each interned value is stored as a [`ThinArc`] whose allocation header caches the
/// `ahash` of the bytes (the `u64` header) alongside the bytes themselves (the `u8` slice).
///
/// Caching the hash in the header lets [`ShardedSet::remove_if_needed`] and the table-growth
/// rehash closure read the precomputed hash instead of re-hashing up to ~190 bytes, and shrinks
/// every table slot from a 16-byte fat `Arc<[u8]>` to an 8-byte thin pointer (denser table,
/// better cache behaviour).
pub(crate) type InternedArc = ThinArc<u64, u8>;

type LockedShard = HashTable<InternedArc>;
type Shard = Mutex<LockedShard>;

#[derive(Debug, Default, Clone, Copy)]
pub struct MemoryUsage {
    pub len: usize,
    pub capacity: usize,
}

pub(crate) struct ShardedSet {
    pub(crate) shift: usize,
    pub(crate) hash_builder: ahash::RandomState,
    pub(crate) shards: Box<[Shard]>,
}

impl ShardedSet {
    fn get_hash_and_shard(&self, value: &[u8]) -> (u64, MutexGuard<'_, LockedShard>) {
        // hash before locking
        let hash = self.hash_builder.hash_one(value);
        (hash, self.get_shard_for_hash(hash))
    }

    /// Locks and returns the shard that a value with the given `hash` belongs to.
    ///
    /// This lets the drop path locate a shard from the hash cached in the [`ThinArc`] header
    /// without re-hashing the bytes.
    fn get_shard_for_hash(&self, hash: u64) -> MutexGuard<'_, LockedShard> {
        // copied from https://github.com/xacrimon/dashmap/blob/366ce7e7872866a06de66eb95002fa6cf2c117a7/src/lib.rs#L419
        let idx = ((hash << 7) >> self.shift) as usize;
        self.shards[idx].lock()
    }

    /// Returns the hash stored in the [`ThinArc`] header instead of re-hashing the bytes.
    ///
    /// Used as the rehash closure on table growth/shrink so the slice bytes are never re-hashed.
    fn hasher(&self, value: &InternedArc) -> u64 {
        value.header.header
    }

    pub(crate) fn get_from_existing_ref(&self, value: &[u8]) -> Option<InternedArc> {
        let (hash, shard) = self.get_hash_and_shard(value);
        shard
            .find(hash, |o| {
                std::ptr::addr_eq(o.slice.as_ptr(), value.as_ptr())
            })
            .cloned()
    }

    pub(crate) fn get_or_insert(&self, value: &[u8]) -> InternedArc {
        let (hash, mut shard) = self.get_hash_and_shard(value);

        shard
            .entry(hash, |o| o.slice.eq(value), |o| self.hasher(o))
            // reuse the hash we already computed for the query bytes
            .or_insert_with(|| ThinArc::from_header_and_slice(hash, value))
            .get()
            .clone()
    }

    /// Only try to remove values from the pool when the reference count is two
    /// one for the given [value] and another for the reference in the pool
    pub(crate) fn remove_if_needed(&self, value: &InternedArc) {
        // one count for `value` and one for the entry in our pool
        const MINIMUM_STRONG_COUNT: usize = 2;

        // lock-free fast path: if there are other live owners we can't be the last,
        // so there is nothing to remove (and no need to take the shard lock or re-hash).
        if ThinArc::strong_count(value) > MINIMUM_STRONG_COUNT {
            return;
        }

        // Read the shard index from the hash cached in the header instead of re-hashing
        // the (up to ~190 byte) slice. The stored hash is exactly the one that was used to
        // place this entry, so it locates the same shard and bucket.
        let hash = value.header.header;
        let mut shard = self.get_shard_for_hash(hash);

        // Find the entry by pointer identity on the byte payload (data pointer), matching the
        // public pointer-identity contract used elsewhere.
        let Ok(entry) = shard.find_entry(hash, |o| {
            std::ptr::addr_eq(o.slice.as_ptr(), value.slice.as_ptr())
        }) else {
            return;
        };

        // check again under the lock in case the value has been cloned in the meantime
        if ThinArc::strong_count(entry.get()) > MINIMUM_STRONG_COUNT {
            return;
        }

        entry.remove();
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub(crate) fn len(&self) -> usize {
        self.shards.iter().map(|o| o.lock().len()).sum()
    }

    pub(crate) fn capacity(&self) -> usize {
        self.shards.iter().map(|o| o.lock().capacity()).sum()
    }

    pub(crate) fn get_memory_usage(&self) -> MemoryUsage {
        self.shards
            .iter()
            .map(|o| {
                let o = o.lock();
                MemoryUsage {
                    len: o.len(),
                    capacity: o.capacity(),
                }
            })
            .reduce(|acc, o| MemoryUsage {
                len: acc.len + o.len,
                capacity: acc.capacity + o.capacity,
            })
            .unwrap_or_default()
    }

    pub(crate) fn shrink_to_fit(&self) {
        for shard in self.shards.iter() {
            shard.lock().shrink_to_fit(|o| self.hasher(o));
        }
    }
}

impl Default for ShardedSet {
    fn default() -> Self {
        // copied from https://github.com/xacrimon/dashmap/blob/366ce7e7872866a06de66eb95002fa6cf2c117a7/src/lib.rs#L63
        static DEFAULT_SHARDS_COUNT: LazyLock<usize> = LazyLock::new(|| {
            (std::thread::available_parallelism().map_or(1, usize::from) * 4).next_power_of_two()
        });

        // copied from https://github.com/xacrimon/dashmap/blob/366ce7e7872866a06de66eb95002fa6cf2c117a7/src/lib.rs#L269
        let shift =
            (std::mem::size_of::<usize>() * 8) - DEFAULT_SHARDS_COUNT.trailing_zeros() as usize;

        Self {
            shift,
            hash_builder: Default::default(),
            shards: (0..*DEFAULT_SHARDS_COUNT)
                .map(|_| Default::default())
                .collect(),
        }
    }
}

pub(crate) static POOL: LazyLock<ShardedSet> = LazyLock::new(Default::default);

pub fn is_empty() -> bool {
    POOL.is_empty()
}

pub fn len() -> usize {
    POOL.len()
}

pub fn capacity() -> usize {
    POOL.capacity()
}

pub fn get_memory_usage() -> MemoryUsage {
    POOL.get_memory_usage()
}

pub fn shrink_to_fit() {
    POOL.shrink_to_fit();
}
