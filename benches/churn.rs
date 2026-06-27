use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use intern_mint::Interned;

include!("./random_strings_pool.rs");

// Create then immediately drop each interned value. Each value's strong count
// falls to the removal threshold on drop, so this drives `remove_if_needed` (the
// removal/drop path) on EVERY drop -- the path the `comparison` bench never
// reaches, because it parks handles in a map at refcount >= 2. This is where
// caching the hash in the allocation header pays off: the baseline re-hashes the
// whole slice on each drop just to locate the shard.
fn create_drop() {
    for &s in POOL {
        let interned = Interned::new(s.as_bytes());
        black_box(&interned);
        drop(interned);
    }
}

fn benchmark(c: &mut Criterion) {
    // Single-threaded: isolates the per-drop CPU cost (the re-hash), no lock contention.
    c.bench_function("churn-st", |b| b.iter(create_drop));
    // Multi-threaded: same churn on every core at once (contended removal).
    c.bench_function("churn-mt", |b| b.iter(|| rayon::broadcast(|_| create_drop())));
}

criterion_group! {
  name = benches;
  config = Criterion::default().sample_size(1000);
  targets = benchmark
}
criterion_main!(benches);
