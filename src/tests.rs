use std::hash::{BuildHasher, Hasher};

use parking_lot::Mutex;
use serial_test::serial;
use triomphe::Arc;

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
        let _a = Interned::new(b"a");
        let _a = Interned::new(b"bit");
        let _a = Interned::new(b"more");
        let _a = Interned::new(b"allocations");
        let _a = Interned::new(b"so");
        let _a = Interned::new(b"we");
        let _a = Interned::new(b"won't");
        let _a = Interned::new(b"use");
        let _a = Interned::new(b"the");
        let _a = Interned::new(b"same");
        let _a = Interned::new(b"address");

        let interned = Interned::new(b"hello!");
        (hash_builder.hash_one(&interned), hash_data(&interned))
    };
    verify_empty();

    assert_ne!(ptr_hash_1, data_hash_1);
    assert_ne!(ptr_hash_1, ptr_hash_2);

    assert_ne!(ptr_hash_2, data_hash_2);
    assert_eq!(data_hash_1, data_hash_2);
}

#[test]
#[serial]
#[cfg(feature = "databuf")]
fn databuf_encode_decode() {
    use databuf::{Decode, Encode, config::num::LE};

    let a = Interned::new(b"hello");
    let encoded = a.to_bytes::<LE>();
    let b = Interned::from_bytes::<LE>(&encoded).unwrap();
    assert_eq!(a.as_ptr(), b.as_ptr());
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
