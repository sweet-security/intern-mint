use std::{ops::Deref, sync::LazyLock};

use hashbrown::HashTable;
use parking_lot::{Mutex, MutexGuard};
use triomphe::Arc;

type LockedShard = HashTable<Arc<[u8]>>;
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
        // copied from https://github.com/xacrimon/dashmap/blob/366ce7e7872866a06de66eb95002fa6cf2c117a7/src/lib.rs#L419
        let idx = ((hash << 7) >> self.shift) as usize;
        let shard = self.shards[idx].lock();
        (hash, shard)
    }

    fn hasher(&self, value: &Arc<[u8]>) -> u64 {
        self.hash_builder.hash_one(value.deref())
    }

    pub(crate) fn get_from_existing_ref(&self, value: &[u8]) -> Option<Arc<[u8]>> {
        let (hash, shard) = self.get_hash_and_shard(value);
        shard
            .find(hash, |o| std::ptr::addr_eq(o.as_ptr(), value.as_ptr()))
            .cloned()
    }

    pub(crate) fn get_or_insert(&self, value: &[u8]) -> Arc<[u8]> {
        let (hash, mut shard) = self.get_hash_and_shard(value);

        shard
            .entry(hash, |o| o.deref() == value, |o| self.hasher(o))
            .or_insert_with(|| Arc::from(value))
            .get()
            .clone()
    }

    /// Only try to remove values from the pool when the reference count is two
    /// one for the given [value] and another for the reference in the pool
    pub(crate) fn remove_if_needed(&self, value: &Arc<[u8]>) {
        // one count for `value` and one for the entry in our pool
        const MINIMUM_STRONG_COUNT: usize = 2;

        if Arc::strong_count(value) > MINIMUM_STRONG_COUNT {
            return;
        }

        let (hash, mut shard) = self.get_hash_and_shard(value);

        let Ok(entry) = shard.find_entry(hash, |o| std::ptr::addr_eq(o.as_ptr(), value.as_ptr()))
        else {
            return;
        };

        // check again in case the value has been cloned
        if Arc::strong_count(entry.get()) > MINIMUM_STRONG_COUNT {
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

/// Returns `true` if the pool contains no interned values.
///
/// # Atomicity
///
/// This is a best-effort point-in-time snapshot. Each shard is locked independently and
/// released before the next is queried, so the result may not reflect an atomic view of
/// the pool. Under concurrent mutation (interning or dropping values), the returned value
/// may not be self-consistent — for example, the pool may appear non-empty even if all
/// values were dropped before this call returns, or vice versa.
pub fn is_empty() -> bool {
    POOL.is_empty()
}

/// Returns the total number of interned values currently held in the pool.
///
/// # Atomicity
///
/// This is a best-effort point-in-time snapshot. Each shard is locked independently and
/// released before the next is queried, so the result may not reflect an atomic view of
/// the pool. Under concurrent mutation (interning or dropping values), the returned count
/// may not be self-consistent — values may be added or removed between shard acquisitions,
/// causing the sum to be transiently higher or lower than any real instantaneous count.
pub fn len() -> usize {
    POOL.len()
}

/// Returns the total hash-table slot capacity across all shards.
///
/// # Atomicity
///
/// This is a best-effort point-in-time snapshot. Each shard is locked independently and
/// released before the next is queried, so the result may not reflect an atomic view of
/// the pool. Under concurrent mutation or rehashing, the returned capacity may not be
/// self-consistent with [`len`] or with itself across repeated calls.
pub fn capacity() -> usize {
    POOL.capacity()
}

/// Returns a [`MemoryUsage`] snapshot of the pool's current entry count and slot capacity.
///
/// # Atomicity
///
/// This is a best-effort point-in-time snapshot. Each shard is locked independently and
/// released before the next is queried, so the result may not reflect an atomic view of
/// the pool. Under concurrent mutation or rehashing, the fields of the returned
/// [`MemoryUsage`] (e.g. `len` and `capacity`) may not be mutually self-consistent —
/// they may have been sampled from different pool states.
pub fn get_memory_usage() -> MemoryUsage {
    POOL.get_memory_usage()
}

pub fn shrink_to_fit() {
    POOL.shrink_to_fit();
}
