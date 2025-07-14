<p align="center">
  <img src="https://github.com/sweet-security/intern-mint/blob/5eab157452131b7bda044a95f349e660b3a44335/logo.png?raw=true" alt="intern-mint" width="350">
</p>

[![version](https://img.shields.io/crates/v/intern-mint)](https://crates.io/crates/intern-mint) [![documentation](https://docs.rs/intern-mint/badge.svg)](https://docs.rs/intern-mint) [![downloads](https://img.shields.io/crates/d/intern-mint)](https://crates.io/crates/intern-mint)

## TL;DR

intern-mint is an implementation of byte slice interning.

## About

Slice interning is a memory management technique that stores identical slices once in a slice pool.

This can potentially save memory and avoid allocations in environments where data is repetitive.

## Technical details

Slices are kept as `Arc<[u8]>`s using the [triomphe](https://github.com/Manishearth/triomphe) crate for a smaller footprint.

The `Arc`s are then stored in a global static pool implemented as a dumbed-down version of [DashMap](https://github.com/xacrimon/dashmap).
The pool consists of `N` shards (dependent on [available_parallelism](https://doc.rust-lang.org/beta/std/thread/fn.available_parallelism.html)) of [hashbrown](https://github.com/rust-lang/hashbrown) hash-tables, sharded by the slices' hashes, to avoid locking the entire table for each lookup.

When a slice is dropped, the total reference count is checked, and the slice is removed from the pool if needed.

## Interned and BorrowedInterned

`Interned` type is the main type offered by this crate, responsible for interning slices.

There is also `&BorrowedInterned` to pass around instead of cloning `Interned` instances when not needed,
and in order to avoid passing `&Interned` which will require double-dereference to access the data.

## Examples

Same data will be held in the same address

```rust
use intern_mint::Interned;

let a = Interned::new(b"hello");
let b = Interned::new(b"hello");

assert_eq!(a.as_ptr(), b.as_ptr());
```

`&BorrowedInterned` can be used with hash-maps

Note that the pointer is being used for hashing and comparing (see `Hash` and `PartialEq` trait implementations)\
as opposed to hashing and comparing the actual data - because the pointers are unique for the same data as long as it "lives" in memory

```rust
use intern_mint::{BorrowedInterned, Interned};

let map = std::collections::HashMap::<Interned, u64>::from_iter([(Interned::new(b"key"), 1)]);

let key = Interned::new(b"key");
assert_eq!(map.get(&key), Some(&1));

let borrowed_key: &BorrowedInterned = &key;
assert_eq!(map.get(borrowed_key), Some(&1));
```

`&BorrowedInterned` can be used with btree-maps

```rust
use intern_mint::{BorrowedInterned, Interned};

let map = std::collections::BTreeMap::<Interned, u64>::from_iter([(Interned::new(b"key"), 1)]);

let key = Interned::new(b"key");
assert_eq!(map.get(&key), Some(&1));

let borrowed_key: &BorrowedInterned = &key;
assert_eq!(map.get(borrowed_key), Some(&1));
```

## Additional features

The following features are available:

- `bstr` to add some type conversions, and the `Debug` and `Display` traits by using the [bstr](https://github.com/BurntSushi/bstr) crate - enabled by default
- `serde` to add the `Serialize` and `Deserialize` traits provided by the [serde](https://github.com/serde-rs/serde) crate - disabled by default
- `databuf` to add the `Encode` and `Decode` traits provided by the [databuf](https://github.com/nurmohammed840/databuf.rs) crate - disabled by default
