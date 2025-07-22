use std::{collections::HashMap, hash::Hash, sync::LazyLock};

use criterion::{Criterion, criterion_group, criterion_main};

include!("./random_strings_pool.rs");

#[inline]
fn use_generic<T: Eq + Hash + Clone>(intern: fn(&str) -> T) -> u32 {
    let mut map = HashMap::<T, u32>::with_capacity(POOL.len());

    for i in 0..2 {
        for &s in POOL {
            let interned = intern(s);
            let value = map.entry(interned).or_default();
            *value += i;
        }
    }

    let mut sum = 0;

    for key in map.keys() {
        let key = key.clone();
        sum += map.get(&key).copied().unwrap_or_default();
    }

    sum
}

fn use_intern_mint() -> u32 {
    use intern_mint::Interned;
    use_generic(|o| Interned::from(o))
}

fn use_internment() -> u32 {
    use internment::ArcIntern;
    use_generic(|o| ArcIntern::<String>::from_ref(o))
}

fn use_intern_arc() -> u32 {
    use intern_arc::HashInterner;
    static INTERN_ARC_POOL: LazyLock<HashInterner<str>> = LazyLock::new(Default::default);
    use_generic(|o| INTERN_ARC_POOL.intern_ref(o))
}

fn use_multithreaded(to_test: fn() -> u32) {
    let _data = rayon::broadcast(|_| to_test());
}

fn benchmark(c: &mut Criterion) {
    rayon::ThreadPoolBuilder::new()
        .num_threads(
            std::thread::available_parallelism()
                .map(|o| o.get().div_ceil(2))
                .unwrap_or(1),
        )
        .build_global()
        .unwrap();

    c.bench_function("intern-mint", |b| {
        b.iter(|| use_multithreaded(use_intern_mint))
    });

    c.bench_function("internment", |b| {
        b.iter(|| use_multithreaded(use_internment))
    });

    c.bench_function("intern-arc", |b| {
        b.iter(|| use_multithreaded(use_intern_arc))
    });
}

criterion_group! {
  name = benches;
  config = Criterion::default().sample_size(10000);
  targets = benchmark
}
criterion_main!(benches);
