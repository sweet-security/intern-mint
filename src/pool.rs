use std::sync::LazyLock;

use hashbrown::HashTable;
use parking_lot::{Mutex, MutexGuard};

use crate::entry::Entry;

type LockedShard = HashTable<Entry>;
type Shard = Mutex<LockedShard>;

/// A point-in-time snapshot of the pool's memory usage.
///
/// Fields are collected by locking each shard independently, so the result is a
/// best-effort snapshot, not an atomic view — values may not be mutually consistent
/// under concurrent mutation.
///
/// The struct is `#[non_exhaustive]` so that adding fields in the future is not a
/// breaking change; construct it via [`get_memory_usage`] rather than a struct literal.
#[non_exhaustive]
#[derive(Debug, Default, Clone, Copy)]
pub struct MemoryUsage {
    /// Number of distinct interned entries currently alive in the pool.
    pub len: usize,
    /// Number of hash-table slots allocated across all shards (capacity in *entries*,
    /// not bytes).
    pub capacity: usize,
    /// Total payload size in **bytes**: the sum of `entry.len()` over every live entry.
    ///
    /// This is computed by iterating all entries under each shard's lock (O(n)), which
    /// is acceptable for an introspection/diagnostic call.
    pub bytes: usize,
}

pub(crate) struct ShardedSet {
    pub(crate) shift: usize,
    pub(crate) hash_builder: ahash::RandomState,
    pub(crate) shards: Box<[Shard]>,
}

impl ShardedSet {
    fn get_shard(&self, hash: u64) -> MutexGuard<'_, LockedShard> {
        // copied from https://github.com/xacrimon/dashmap/blob/366ce7e7872866a06de66eb95002fa6cf2c117a7/src/lib.rs#L419
        let idx = ((hash << 7) >> self.shift) as usize;
        self.shards[idx].lock()
    }

    fn get_hash_and_shard(&self, value: &[u8]) -> (u64, MutexGuard<'_, LockedShard>) {
        // hash before locking
        let hash = self.hash_builder.hash_one(value);
        (hash, self.get_shard(hash))
    }

    pub(crate) fn get_from_existing_ref(&self, value: &[u8]) -> Option<Entry> {
        let (hash, shard) = self.get_hash_and_shard(value);
        shard
            .find(hash, |o| std::ptr::eq(o.data_ptr(), value.as_ptr()))
            // Safety: the shard is locked
            .map(|o| unsafe { o.acquire() })
    }

    pub(crate) fn get_or_insert(&self, value: &[u8]) -> Entry {
        let (hash, mut shard) = self.get_hash_and_shard(value);

        // Safety: entries are only accessed while the shard is locked
        let entry = shard
            .entry(
                hash,
                |o| unsafe { o.data() } == value,
                |o| unsafe { o.hash() },
            )
            .or_insert_with(|| Entry::new(value, hash));

        // Safety: the shard is locked
        unsafe { entry.get().acquire() }
    }

    /// Gives up a handle, removing `entry` from the pool if it was the last one
    ///
    /// # Safety
    ///
    /// The caller must own the handle being given up, and must not use it after this call
    pub(crate) unsafe fn release(&self, entry: Entry) {
        // read before releasing, since another thread may free the entry right after
        // Safety: the handle being given up keeps the entry alive until then
        let hash = unsafe { entry.hash() };

        // Safety: guaranteed by the caller
        if !unsafe { entry.release() } {
            return;
        }

        // `entry` may be dangling from here on, so it's only compared by address
        let mut shard = self.get_shard(hash);

        let Ok(found) = shard.find_entry(hash, |o| *o == entry) else {
            // another thread already removed it
            return;
        };

        // check again in case a new handle was acquired before the shard was locked
        // Safety: the shard is locked
        if !unsafe { found.get().is_unused() } {
            return;
        }

        let (removed, _) = found.remove();
        // free outside of the lock
        drop(shard);
        // Safety: the entry was removed with no handles left, so nothing can reach it anymore
        unsafe { removed.free() };
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.shards.iter().all(|s| s.lock().is_empty())
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
                let bytes: usize = o.iter().map(|entry| entry.len()).sum();
                MemoryUsage {
                    len: o.len(),
                    capacity: o.capacity(),
                    bytes,
                }
            })
            .reduce(|acc, o| MemoryUsage {
                len: acc.len + o.len,
                capacity: acc.capacity + o.capacity,
                bytes: acc.bytes + o.bytes,
            })
            .unwrap_or_default()
    }

    pub(crate) fn shrink_to_fit(&self) {
        for shard in self.shards.iter() {
            // Safety: the shard is locked
            shard.lock().shrink_to_fit(|o| unsafe { o.hash() });
        }
    }
}

impl Default for ShardedSet {
    fn default() -> Self {
        // copied from https://github.com/xacrimon/dashmap/blob/366ce7e7872866a06de66eb95002fa6cf2c117a7/src/lib.rs#L63
        static DEFAULT_SHARDS_COUNT: LazyLock<usize> = LazyLock::new(|| {
            (std::thread::available_parallelism().map_or(1, usize::from) * 4).next_power_of_two()
        });

        // based on https://github.com/xacrimon/dashmap/blob/366ce7e7872866a06de66eb95002fa6cf2c117a7/src/lib.rs#L269
        // using the width of the u64 hash rather than usize, which is narrower on 32-bit targets
        let shift = (u64::BITS - DEFAULT_SHARDS_COUNT.trailing_zeros()) as usize;

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
