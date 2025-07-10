use std::{
    borrow::Borrow,
    cmp::Ordering,
    hash::{Hash, Hasher},
    ops::Deref,
    sync::LazyLock,
};

use hashbrown::hash_table::{Entry, HashTable};
use parking_lot::{Mutex, MutexGuard};
use triomphe::Arc;

type LockedShard = HashTable<Arc<[u8]>>;
type Shard = Mutex<LockedShard>;

struct ShardedSet {
    shift: usize,
    hash_builder: ahash::RandomState,
    shards: Box<[Shard]>,
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

    pub fn get_or_insert(&self, value: &[u8]) -> Arc<[u8]> {
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
    pub fn remove_if_needed(&self, value: &Arc<[u8]>) {
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

static POOL: LazyLock<ShardedSet> = LazyLock::new(Default::default);

#[derive(Clone, Eq)]
#[repr(transparent)]
pub struct Interned(Arc<[u8]>);

impl Interned {
    pub fn new(value: &[u8]) -> Self {
        Self(POOL.get_or_insert(value))
    }
}

impl Drop for Interned {
    fn drop(&mut self) {
        POOL.remove_if_needed(&self.0);
    }
}

impl Deref for Interned {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

impl PartialEq for Interned {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.as_ptr(), other.as_ptr())
    }
}

impl Hash for Interned {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_ptr().hash(state);
    }
}

impl PartialOrd for Interned {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Interned {
    fn cmp(&self, other: &Self) -> Ordering {
        self.deref().cmp(other.deref())
    }
}

impl From<&[u8]> for Interned {
    fn from(value: &[u8]) -> Self {
        Interned::new(value)
    }
}

impl Borrow<BorrowedInterned> for Interned {
    fn borrow(&self) -> &BorrowedInterned {
        BorrowedInterned::new(self.deref())
    }
}

impl AsRef<BorrowedInterned> for Interned {
    fn as_ref(&self) -> &BorrowedInterned {
        BorrowedInterned::new(self.deref())
    }
}

#[derive(Eq)]
#[repr(transparent)]
pub struct BorrowedInterned([u8]);

impl BorrowedInterned {
    fn new(value: &[u8]) -> &BorrowedInterned {
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

#[test]
fn sanity() {
    let verify_empty = || {
        let len = POOL.shards.iter().map(|o| o.lock().len()).sum::<usize>();
        assert_eq!(len, 0);
    };
    verify_empty();

    {
        let a = Interned::new(b"hello");
        let b = Interned::new(b"hello");

        assert_eq!(a.as_ptr(), b.as_ptr());
    }
    verify_empty();

    {
        let a = Interned::new(b"hello");
        let b = Interned::new(b"bye");
        let c = Interned::new(b"why");
        let d = Interned::new(b"just");
        let e = Interned::new(b"because");

        assert_ne!(a.as_ptr(), b.as_ptr());
        assert_ne!(b.as_ptr(), c.as_ptr());
        assert_ne!(c.as_ptr(), d.as_ptr());
        assert_ne!(d.as_ptr(), e.as_ptr());
    }
    verify_empty();

    {
        let a = Interned::new(b"hello");
        let b = a.clone();
        let c = a.clone();
        let d = a.clone();
        let e = a.clone();

        assert_eq!(a.as_ptr(), b.as_ptr());
        assert_eq!(b.as_ptr(), c.as_ptr());
        assert_eq!(c.as_ptr(), d.as_ptr());
        assert_eq!(d.as_ptr(), e.as_ptr());
    }
    verify_empty();

    {
        const LEN: usize = 1024;

        let arcs = Arc::new(Mutex::new(Vec::<Interned>::new()));

        let threads = (0..LEN)
            .map(|_| {
                std::thread::spawn({
                    let arcs = arcs.clone();
                    move || {
                        let arced = Interned::new(b"hello");
                        arcs.lock().push(arced);
                    }
                })
            })
            .collect::<Vec<_>>();

        for thread in threads {
            _ = thread.join();
        }

        {
            let arcs = arcs.lock();
            assert_eq!(arcs.len(), LEN);
            assert!(
                arcs.iter()
                    .skip(1)
                    .all(|o| std::ptr::eq(arcs[0].as_ptr(), o.as_ptr()))
            );
        }
    }
    verify_empty();

    {
        use std::collections::HashMap;

        let map = HashMap::<Interned, u64>::from_iter([(b"key".as_ref().into(), 1)]);

        let key = Interned::new(b"key");
        assert_eq!(map.get(&key), Some(&1));

        let borrowed_key = key.as_ref();
        assert_eq!(map.get(borrowed_key), Some(&1));

        let unknown_key = Interned::new(b"unknown_key");
        assert_eq!(map.get(&unknown_key), None);

        let borrowed_unknown_key = unknown_key.as_ref();
        assert_eq!(map.get(borrowed_unknown_key), None);
    }
    verify_empty();
}
