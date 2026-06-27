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

    /// Performs the final reference-count decrement for a dropped [Interned] handle, evicting the
    /// pool's entry if this was the last external handle.
    ///
    /// Ownership of `value` (the [Arc] taken out of the dropping handle) is moved in, and the
    /// decrement is performed *under the shard lock* by dropping `value` before the guard's scope
    /// ends. This is essential for correctness: it serializes the decrement against the clones done
    /// in `get_or_insert` / `get_from_existing_ref` (which also hold the shard lock), so a
    /// `strong_count` read taken under the lock is authoritative and cannot race with another
    /// dropper.
    ///
    /// The previous implementation read `strong_count` *without* the lock and bailed early when it
    /// looked larger than the minimum. That is a TOCTOU: two threads dropping the last two handles
    /// to the same data could both observe `count > 2` and bail, then both decrement, permanently
    /// orphaning the entry. Doing the decrement under the lock closes that window.
    ///
    /// Trade-off (rehash on drop): with the unlocked fast path gone, every drop now hashes the full
    /// slice to locate its shard and then takes the shard lock. For large slices this dominates the
    /// drop cost (benchmarked at ~250ns/drop for a 4 KiB slice vs ~14ns for 8 bytes). Caching the
    /// hash inside [Interned] removes it (a 4 KiB drop drops to ~19ns) but costs +8 bytes on *every*
    /// handle and widens the type out of `repr(transparent)`; for an interning crate whose value is
    /// memory deduplication, that per-handle cost was judged not worth it here, and threading the
    /// hash through cleanly belongs with the `get_or_insert` work in a later PR. Caching was
    /// therefore left out; revisit it there if drop-heavy large-slice workloads matter.
    ///
    /// [Interned]: crate::Interned
    pub(crate) fn remove_on_last_drop(&self, value: Arc<[u8]>) {
        // one count for `value` (this dropping handle) and one for the entry in our pool
        const MINIMUM_STRONG_COUNT: usize = 2;

        let (hash, mut shard) = self.get_hash_and_shard(&value);

        // Under the shard lock the count is authoritative. If only this handle and the pool hold a
        // reference, this is the last external handle, so evict the pool's entry (drops the pool's
        // `Arc`, 2 -> 1). Otherwise other handles remain and we leave the entry in place.
        if Arc::strong_count(&value) == MINIMUM_STRONG_COUNT
            && let Ok(entry) =
                shard.find_entry(hash, |o| std::ptr::addr_eq(o.as_ptr(), value.as_ptr()))
        {
            entry.remove();
        }

        // Perform this handle's decrement while STILL holding the shard guard, so the final
        // refcount mutation is serialized with every other mutation on this shard. `shard` is
        // dropped (releasing the lock) only after this statement.
        drop(value);
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
