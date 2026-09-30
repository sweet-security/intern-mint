use std::{
    hash::{BuildHasher, Hasher},
    sync::Arc,
};

use parking_lot::Mutex;
use serial_test::serial;

use crate::{BorrowedInterned, Interned, pool};

fn verify_empty() {
    // after default interned is used for the first time, it's kept forever in the pool
    // create a default instance in case it didn't exist before
    let _a = Interned::default();
    assert!(pool::len() == 1);
}

#[test]
#[serial]
fn same_data_same_ptr() {
    {
        let a = Interned::new(b"hello");
        let b = Interned::new(b"hello");

        assert_eq!(a.as_ptr(), b.as_ptr());
        #[cfg(feature = "bstr")]
        assert_eq!(a, b);
    }
    verify_empty();
}

#[test]
#[serial]
fn different_data_different_ptr() {
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

        #[cfg(feature = "bstr")]
        {
            assert_ne!(a, b);
            assert_ne!(b, c);
            assert_ne!(c, d);
            assert_ne!(d, e);
        }
    }
    verify_empty();
}

#[test]
#[serial]
fn cloned_data_same_ptr() {
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

        #[cfg(feature = "bstr")]
        {
            assert_eq!(a, b);
            assert_eq!(b, c);
            assert_eq!(c, d);
            assert_eq!(d, e);
        }
    }
    verify_empty();
}

#[test]
#[serial]
fn same_data_multithreaded_same_ptr() {
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
                    .all(|o| std::ptr::addr_eq(arcs[0].as_ptr(), o.as_ptr()))
            );
        }
    }
    verify_empty();
}

#[test]
#[serial]
fn multithreaded_drop() {
    {
        const LEN: usize = 1024;

        let threads = (0..LEN)
            .map(|_| {
                std::thread::spawn({
                    || {
                        let arced = Interned::new(b"hello");
                        drop(arced)
                    }
                })
            })
            .collect::<Vec<_>>();

        for thread in threads {
            _ = thread.join();
        }
    }
    verify_empty();
}

#[test]
#[serial]
fn concurrent_last_drops() {
    // all threads drop their clone of the same value at once, so the last handles are released
    // concurrently, which used to leave entries behind in the pool
    {
        const THREADS: usize = 8;
        const ITERATIONS: usize = if cfg!(miri) { 50 } else { 20_000 };

        let barrier = std::sync::Barrier::new(THREADS);
        let slots = (0..THREADS)
            .map(|_| Mutex::new(None::<Interned>))
            .collect::<Vec<_>>();

        std::thread::scope(|scope| {
            for thread in 0..THREADS {
                let (barrier, slots) = (&barrier, &slots);
                scope.spawn(move || {
                    for i in 0..ITERATIONS {
                        if thread == 0 {
                            let interned = Interned::new(format!("value {i}").as_bytes());
                            for slot in slots {
                                *slot.lock() = Some(interned.clone());
                            }
                        }
                        barrier.wait();
                        let interned = slots[thread].lock().take();
                        barrier.wait();
                        drop(interned);
                        barrier.wait();
                    }
                });
            }
        });
    }
    verify_empty();
}

#[test]
#[serial]
fn map_usage_with_borrow() {
    {
        use std::collections::HashMap;

        let map = HashMap::<Interned, u64>::from_iter([(b"key".as_ref().into(), 1)]);

        let key = Interned::new(b"key");
        assert_eq!(map.get(&key), Some(&1));

        let borrowed_key: &BorrowedInterned = &key;
        assert_eq!(map.get(borrowed_key), Some(&1));

        let unknown_key = Interned::new(b"unknown_key");
        assert_eq!(map.get(&unknown_key), None);

        let borrowed_unknown_key = unknown_key.as_ref();
        assert_eq!(map.get(borrowed_unknown_key), None);
    }
    verify_empty();
}

#[test]
#[serial]
fn btree_usage_with_borrow() {
    {
        use std::collections::BTreeMap;

        let map = BTreeMap::<Interned, u64>::from_iter([(b"key".as_ref().into(), 1)]);

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

#[test]
#[serial]
fn re_intern_borrow_same_ptr() {
    {
        let interned = Interned::new(b"hello!");
        let interned_from_borrow = interned.as_ref().intern();
        assert_eq!(interned.as_ptr(), interned_from_borrow.as_ptr());
    }
    verify_empty();
}

#[test]
#[serial]
fn validate_data_hash() {
    let hash_builder = ahash::RandomState::new();

    let hash_data = |data: &Interned| {
        let mut hasher = hash_builder.build_hasher();
        data.hash_data(&mut hasher);
        hasher.finish()
    };

    let (ptr_hash_1, data_hash_1) = {
        let interned = Interned::new(b"hello!");
        (hash_builder.hash_one(&interned), hash_data(&interned))
    };
    verify_empty();

    let (ptr_hash_2, data_hash_2) = {
        let interned = Interned::new(b"hello!");
        (hash_builder.hash_one(&interned), hash_data(&interned))
    };
    verify_empty();

    assert_ne!(ptr_hash_1, data_hash_1);

    assert_ne!(ptr_hash_2, data_hash_2);
    assert_eq!(data_hash_1, data_hash_2);
}

#[test]
#[serial]
#[cfg(feature = "serde")]
fn serde() {
    let a = Interned::new(b"hello");
    let serialized = serde_json::to_string(&a).expect("serialize");
    let b = serde_json::from_str::<Interned>(&serialized).expect("deserialize");
    assert_eq!(a.as_ptr(), b.as_ptr());
}

#[test]
fn thin_handle() {
    assert_eq!(size_of::<Interned>(), size_of::<usize>());
    assert_eq!(size_of::<Option<Interned>>(), size_of::<usize>());
}
