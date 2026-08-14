//! Benchmarks for contiguous component storage operations.
//!
//! Direct comparisons use the retained generic storage as a reference.
//! Both implementations enter each measured comparison from equivalent
//! logical states.
//!
//! The push benchmark deliberately prepares both empty storages with capacity
//! for one component before timing begins. The initial allocation is therefore
//! excluded symmetrically, and every measured growth step starts from the same
//! capacity.
//!
//! Population for removal benchmarks is setup work and remains outside the
//! measurement. The shared-access benchmark is BlobVec-only because the
//! retained baseline exposes no equivalent accessor.

mod support;

use std::hint::black_box;

use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use frogbyte_ecs::component::{Component, blobvec::BlobVec};

use crate::support::generic_storage_baseline::GenericStorageBaseline;

#[derive(Clone, Copy)]
struct BenchComponent([u64; 2]);

impl Component for BenchComponent {}

const COMPONENT_COUNTS: [usize; 3] = [128, 1_024, 8_192];

fn component_at(index: usize) -> BenchComponent {
    let value = index as u64;
    BenchComponent([value, !value])
}

fn input_components(count: usize) -> Vec<BenchComponent> {
    (0..count).map(component_at).collect()
}

/// Returns an empty BlobVec whose initial one-element allocation has already
/// happened.
///
/// `push` followed by `pop` leaves the logical length at zero while retaining
/// the first allocation, matching `GenericStorageBaseline::new`.
fn warmed_empty_blobvec() -> BlobVec {
    let mut storage = BlobVec::new::<BenchComponent>();

    storage.push(component_at(0));
    let _ = storage.pop::<BenchComponent>();

    storage
}

fn populated_blobvec(count: usize) -> BlobVec {
    let mut storage = BlobVec::new::<BenchComponent>();

    for index in 0..count {
        storage.push(component_at(index));
    }

    storage
}

fn populated_baseline(count: usize) -> GenericStorageBaseline<BenchComponent> {
    let mut storage = GenericStorageBaseline::new();

    for index in 0..count {
        storage.push(component_at(index));
    }

    storage
}

/// Measures appending a complete column after both storages already own their
/// first one-element allocation.
///
/// This isolates the push path and subsequent growth strategies without giving
/// either implementation a hidden initial-allocation advantage.
fn bench_push_growth(c: &mut Criterion) {
    let mut group = c.benchmark_group("component_storage_push");

    for &count in &COMPONENT_COUNTS {
        group.throughput(Throughput::Elements(count as u64));

        group.bench_with_input(
            BenchmarkId::new("blobvec", format!("{count}_components")),
            &count,
            |b, &count| {
                b.iter_batched_ref(
                    || (warmed_empty_blobvec(), input_components(count)),
                    |(storage, values)| {
                        for &value in values.iter() {
                            storage.push(black_box(value));
                        }
                    },
                    BatchSize::LargeInput,
                );
            },
        );

        group.bench_with_input(
            BenchmarkId::new("generic_baseline", format!("{count}_components")),
            &count,
            |b, &count| {
                b.iter_batched_ref(
                    || {
                        (
                            GenericStorageBaseline::<BenchComponent>::new(),
                            input_components(count),
                        )
                    },
                    |(storage, values)| {
                        for &value in values.iter() {
                            storage.push(black_box(value));
                        }
                    },
                    BatchSize::LargeInput,
                );
            },
        );
    }

    group.finish();
}

/// Measures removing an existing column from the end.
///
/// Population and destruction of the batch state are setup/teardown concerns;
/// the measured operation is only the sequence of `pop` calls.
fn bench_pop_drain(c: &mut Criterion) {
    let mut group = c.benchmark_group("component_storage_pop");

    for &count in &COMPONENT_COUNTS {
        group.throughput(Throughput::Elements(count as u64));

        group.bench_with_input(
            BenchmarkId::new("blobvec", format!("{count}_components")),
            &count,
            |b, &count| {
                b.iter_batched_ref(
                    || populated_blobvec(count),
                    |storage| {
                        for _ in 0..count {
                            black_box(storage.pop::<BenchComponent>());
                        }
                    },
                    BatchSize::LargeInput,
                );
            },
        );

        group.bench_with_input(
            BenchmarkId::new("generic_baseline", format!("{count}_components")),
            &count,
            |b, &count| {
                b.iter_batched_ref(
                    || populated_baseline(count),
                    |storage| {
                        for _ in 0..count {
                            black_box(storage.pop());
                        }
                    },
                    BatchSize::LargeInput,
                );
            },
        );
    }

    group.finish();
}

/// Measures repeatedly removing index zero from a populated column.
///
/// Each operation moves the current final component into the vacated first
/// slot, exercising the non-trivial swap-remove path rather than the
/// last-element fast case.
fn bench_swap_remove_drain(c: &mut Criterion) {
    let mut group = c.benchmark_group("component_storage_swap_remove");

    for &count in &COMPONENT_COUNTS {
        group.throughput(Throughput::Elements(count as u64));

        group.bench_with_input(
            BenchmarkId::new("blobvec", format!("{count}_components")),
            &count,
            |b, &count| {
                b.iter_batched_ref(
                    || populated_blobvec(count),
                    |storage| {
                        for _ in 0..count {
                            black_box(storage.swap_remove::<BenchComponent>(0));
                        }
                    },
                    BatchSize::LargeInput,
                );
            },
        );

        group.bench_with_input(
            BenchmarkId::new("generic_baseline", format!("{count}_components")),
            &count,
            |b, &count| {
                b.iter_batched_ref(
                    || populated_baseline(count),
                    |storage| {
                        for _ in 0..count {
                            black_box(storage.swap_remove(0));
                        }
                    },
                    BatchSize::LargeInput,
                );
            },
        );
    }

    group.finish();
}

/// Measures sequential shared access through BlobVec's type-erased accessor.
///
/// This is intentionally not compared with the generic baseline because that
/// retained implementation has no equivalent public benchmark accessor.
fn bench_get_sweep(c: &mut Criterion) {
    let mut group = c.benchmark_group("component_storage_get_sweep");

    for &count in &COMPONENT_COUNTS {
        let storage = populated_blobvec(count);

        group.throughput(Throughput::Elements(count as u64));

        group.bench_with_input(
            BenchmarkId::new("blobvec", format!("{count}_components")),
            &storage,
            |b, storage| {
                b.iter(|| {
                    for index in 0..count {
                        if let Some(value) = storage.get::<BenchComponent>(index) {
                            black_box(value.0);
                        }
                    }
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    component_storage_benches,
    bench_push_growth,
    bench_pop_drain,
    bench_swap_remove_drain,
    bench_get_sweep,
);
criterion_main!(component_storage_benches);
