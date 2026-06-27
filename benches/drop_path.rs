//! Microbenchmark for the [`Interned`] drop path.
//!
//! After the drop-path race fix, every drop hashes the full slice to locate its shard and takes the
//! shard lock (the old unlocked `strong_count` fast path is gone). This bench measures pure drop
//! throughput for small and large slices so the rehash cost (coverage finding #4) can be evaluated
//! against the alternative of caching the hash inside `Interned`.
//!
//! Each timed iteration drops a freshly-interned, unique handle, so it exercises the
//! last-drop-evicts-the-entry path (`remove_on_last_drop` with `strong_count == 2`), which is the
//! path that now hashes + locks.

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use intern_mint::Interned;

/// Build `count` unique byte vectors of length `len`. Uniqueness guarantees every drop is a real
/// last-drop eviction rather than a no-op refcount decrement.
fn unique_data(count: usize, len: usize) -> Vec<Vec<u8>> {
    (0..count)
        .map(|i| {
            let mut v = vec![0u8; len];
            let bytes = (i as u64).to_ne_bytes();
            let n = bytes.len().min(len);
            v[..n].copy_from_slice(&bytes[..n]);
            v
        })
        .collect()
}

fn bench_drop_for_len(c: &mut Criterion, name: &str, len: usize) {
    // a pool of distinct inputs to cycle through so each setup interns fresh data
    const POOL_SIZE: usize = 4096;
    let data = unique_data(POOL_SIZE, len);
    let mut next = 0usize;

    c.bench_function(name, |b| {
        b.iter_batched(
            || {
                // setup (untimed): intern one unique handle
                let d = &data[next % POOL_SIZE];
                next += 1;
                Interned::new(d)
            },
            // timed: drop the single last handle -> hashes + locks + evicts
            drop,
            BatchSize::SmallInput,
        )
    });
}

fn benchmark(c: &mut Criterion) {
    // small slice: hashing is cheap, so per-drop overhead is dominated by lock + lookup
    bench_drop_for_len(c, "drop_small_8b", 8);
    // large slice: hashing the full slice on every drop is the cost hash-caching would remove
    bench_drop_for_len(c, "drop_large_256b", 256);
    bench_drop_for_len(c, "drop_large_4kb", 4096);
}

criterion_group! {
  name = benches;
  config = Criterion::default();
  targets = benchmark
}
criterion_main!(benches);
