use std::{ops::Deref, sync::LazyLock};

use crossbeam_utils::CachePadded;
use hashbrown::HashTable;
use parking_lot::RwLock;
use triomphe::Arc;

type LockedShard = HashTable<Arc<[u8]>>;
// Cache-padded so adjacent shard locks don't share a cache line (avoids false
// sharing under contention). `CachePadded` derefs to the `RwLock`, so
// `self.shards[idx].read()` / `.write()` work directly.
type Shard = CachePadded<RwLock<LockedShard>>;

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
    /// Compute the hash of `value` and the index of the shard it belongs to.
    /// The caller decides whether to take a read or write lock on the shard.
    fn get_hash_and_idx(&self, value: &[u8]) -> (u64, usize) {
        let hash = self.hash_builder.hash_one(value);
        // copied from https://github.com/xacrimon/dashmap/blob/366ce7e7872866a06de66eb95002fa6cf2c117a7/src/lib.rs#L419
        let idx = ((hash << 7) >> self.shift) as usize;
        (hash, idx)
    }

    fn hasher(&self, value: &Arc<[u8]>) -> u64 {
        self.hash_builder.hash_one(value.deref())
    }

    pub(crate) fn get_from_existing_ref(&self, value: &[u8]) -> Option<Arc<[u8]>> {
        let (hash, idx) = self.get_hash_and_idx(value);
        // `find` takes `&self`, so a shared read lock is enough.
        self.shards[idx]
            .read()
            .find(hash, |o| std::ptr::addr_eq(o.as_ptr(), value.as_ptr()))
            .cloned()
    }

    pub(crate) fn get_or_insert(&self, value: &[u8]) -> Arc<[u8]> {
        let (hash, idx) = self.get_hash_and_idx(value);

        // Hot path: most `Interned::new` calls hit an existing entry. Take a
        // shared read lock so concurrent readers proceed in parallel. Cloning
        // the `Arc` under the read lock bumps its strong count, which keeps a
        // concurrent remover (which only removes under the write lock and
        // re-checks the strong count there) from removing it out from under us.
        if let Some(found) = self.shards[idx]
            .read()
            .find(hash, |o| o.deref() == value)
            .cloned()
        {
            return found;
        }

        // Miss: drop the read lock (the `if let` guard is already released
        // here) and take the write lock to insert. The `entry` API re-checks
        // under the write lock, so a value inserted by another thread between
        // releasing the read lock and acquiring the write lock is found rather
        // than duplicated.
        self.shards[idx]
            .write()
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

        let (hash, idx) = self.get_hash_and_idx(value);

        // Removal happens exclusively under the write lock. This is what makes
        // the read-locked lookup in `get_or_insert` / `get_from_existing_ref`
        // sound: a reader holding the shared read lock has already incremented
        // the strong count before this writer can acquire the exclusive lock,
        // so the re-check below sees the higher count and bails.
        let mut shard = self.shards[idx].write();

        let Ok(entry) = shard.find_entry(hash, |o| std::ptr::addr_eq(o.as_ptr(), value.as_ptr()))
        else {
            return;
        };

        // check again in case the value has been cloned (e.g. by a concurrent
        // reader) between the lock-free fast-path check above and acquiring the
        // write lock.
        if Arc::strong_count(entry.get()) > MINIMUM_STRONG_COUNT {
            return;
        }

        entry.remove();
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub(crate) fn len(&self) -> usize {
        // One shard locked (and released) at a time; never nest shard locks.
        self.shards.iter().map(|o| o.read().len()).sum()
    }

    pub(crate) fn capacity(&self) -> usize {
        // One shard locked (and released) at a time; never nest shard locks.
        self.shards.iter().map(|o| o.read().capacity()).sum()
    }

    pub(crate) fn get_memory_usage(&self) -> MemoryUsage {
        self.shards
            .iter()
            .map(|o| {
                // One shard locked (and released) at a time; never nest locks.
                let o = o.read();
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
            shard.write().shrink_to_fit(|o| self.hasher(o));
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
