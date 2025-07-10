use parking_lot::Mutex;
use triomphe::Arc;

use crate::{interned::Interned, pool::POOL};

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
