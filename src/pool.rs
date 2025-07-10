use std::{ops::Deref, sync::LazyLock};

use hashbrown::{HashTable, hash_table::Entry};
use parking_lot::{Mutex, MutexGuard};
use triomphe::Arc;

type LockedShard = HashTable<Arc<[u8]>>;
type Shard = Mutex<LockedShard>;

pub(crate) struct ShardedSet {
    pub(crate) shift: usize,
    pub(crate) hash_builder: ahash::RandomState,
    pub(crate) shards: Box<[Shard]>,
}

impl ShardedSet {
    fn get_hash_and_shard(&self, value: &[u8]) -> (u64, MutexGuard<LockedShard>) {
        // hash before locking
        let hash = self.hash_builder.hash_one(value);
        // copied from https://github.com/xacrimon/dashmap/blob/366ce7e7872866a06de66eb95002fa6cf2c117a7/src/lib.rs#L419
        let idx = ((hash << 7) >> self.shift) as usize;
        let shard = self.shards[idx].lock();
        (hash, shard)
    }

    pub(crate) fn get_or_insert(&self, value: &[u8]) -> Arc<[u8]> {
        let (hash, mut shard) = self.get_hash_and_shard(value);

        shard
            .entry(
                hash,
                |o| o.deref() == value,
                |o| self.hash_builder.hash_one(o.deref()),
            )
            .or_insert_with(|| Arc::from(value))
            .get()
            .clone()
    }

    /// Only try to remove values from the pool when the reference count is two
    /// one for the given [value] and another for the reference in the pool
    pub(crate) fn remove_if_needed(&self, value: &Arc<[u8]>) {
        const MINIMUM_STRONG_COUNT: usize = 2;

        if Arc::strong_count(value) > MINIMUM_STRONG_COUNT {
            return;
        }

        let (hash, mut shard) = self.get_hash_and_shard(value);

        if let Entry::Occupied(entry) = shard.entry(
            hash,
            |o| std::ptr::addr_eq(o.as_ptr(), value.as_ptr()),
            |o| self.hash_builder.hash_one(o.deref()),
        ) && Arc::strong_count(entry.get()) <= MINIMUM_STRONG_COUNT
        {
            entry.remove();
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub(crate) fn len(&self) -> usize {
        self.shards.iter().map(|o| o.lock().len()).sum()
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
