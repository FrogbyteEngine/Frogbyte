//! Benchmarks comparing the contiguous type-erased component storage
//! [`BlobVec`] with the retained generic storage baseline.
//!
//! Every comparison group runs the same logical workload on both storages from
//! the same logical starting state, so a reported difference is the cost of
//! type erasure plus the cost of the growth strategy: [`BlobVec`] reallocates a
//! single buffer and can keep it in place, while the baseline allocates a new
//! buffer and moves every element into it.
//!
//! The batched workloads keep their storage as Criterion batch state and
//! operate on it through a reference, so building the initial population and
//! releasing the storage afterwards stay outside measurement. Only the baseline
//! allocates in its constructor, so its first single-slot allocation is the one
//! growth step that setup absorbs; every later growth of either storage is
//! measured.
//!
//! Batch state scales with the component count, so the batched workloads use
//! `BatchSize::LargeInput` to bound how much of it Criterion keeps alive at
//! once.
//!
//! The baseline exposes no component accessor, so the shared-access sweep
//! covers the type-erased storage alone and is deliberately not reported as a
//! comparison.

mod support;

use std::hint::black_box;

use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use frogbyte_ecs::component::{blobvec::BlobVec, position_component::Position};

use crate::support::generic_storage_baseline::GenericStorageBaseline;

/// Component counts covering a small, medium, and large column.
///
/// At the largest count the stored components no longer fit in a typical L1
/// data cache, so the workloads also report how each storage behaves once the
/// column is walked from slower memory.
const COMPONENT_COUNTS: [usize; 3] = [128, 1_024, 8_192];

/// Builds the component stored at `index`.
///
/// Values differ per element so a workload cannot be reduced to writing one
/// constant repeatedly.
fn component_at(index: usize) -> Position {
    let base = index as f32;

    Position {
        x: base,
        y: base + 1.0,
        z: base + 2.0,
    }
}

/// Creates a type-erased storage holding `count` components.
///
/// Shared setup for the workloads that measure operations over an existing
/// column; it always runs outside the measured routine.
fn populated_blobvec(count: usize) -> BlobVec {
    let mut storage = BlobVec::new::<Position>();

    for index in 0..count {
        storage.push(component_at(index));
    }

    storage
}

/// Creates a baseline storage holding `count` components, matching the state
/// [`populated_blobvec`] produces.
fn populated_baseline(count: usize) -> GenericStorageBaseline<Position> {
    let mut storage = GenericStorageBaseline::new();

    for index in 0..count {
        storage.push(component_at(index));
    }

    storage
}

/// Appending components into an empty storage, which is how a component column
/// is built and therefore pays every reallocation on the way.
///
/// This is where the two growth strategies differ, so the group reports the
/// per-push cost together with the cost of moving the existing components
/// whenever the buffer has to grow.
fn bench_push_fill(c: &mut Criterion) {
    let mut group = c.benchmark_group("component_storage_push");

    for &count in &COMPONENT_COUNTS {
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(
            BenchmarkId::new("blobvec", format!("{count}_components")),
            &count,
            |b, &count| {
                b.iter_batched_ref(
                    BlobVec::new::<Position>,
                    |storage| {
                        for index in 0..count {
                            storage.push(black_box(component_at(index)));
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
                    GenericStorageBaseline::<Position>::new,
                    |storage| {
                        for index in 0..count {
                            storage.push(black_box(component_at(index)));
                        }
                    },
                    BatchSize::LargeInput,
                );
            },
        );
    }

    group.finish();
}

/// Draining a full column from the end, which is the cost of removing the last
/// component without moving any other component.
///
/// It is the reference point for the swap-removal workload below: the
/// difference between the two reports what filling the resulting hole costs,
/// rather than leaving that cost mixed into a single removal number.
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
                            black_box(storage.pop::<Position>());
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

/// Removing every component from the front with a swap removal, which is the
/// despawn path: each removal moves the current last component into the hole so
/// the column stays contiguous.
///
/// Removing at index zero keeps the moved component far from the removed one,
/// so the workload touches both ends of the column instead of the best case
/// where the removed element is already the last one.
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
                            black_box(storage.swap_remove::<Position>(0));
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

/// Sweeping shared access across a whole column, the read pattern a system
/// performs when it iterates its components in order.
///
/// Every access re-checks the stored type identifier and the length, so this
/// reports the per-component cost of type-erased access together with the
/// memory traffic of walking the column.
fn bench_get_sweep(c: &mut Criterion) {
    let mut group = c.benchmark_group("component_storage_get_sweep");

    for &count in &COMPONENT_COUNTS {
        // Shared access does not mutate the storage, so the same population is
        // reused across iterations instead of being rebuilt per iteration.
        let storage = populated_blobvec(count);

        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(
            BenchmarkId::new("blobvec", format!("{count}_components")),
            &storage,
            |b, storage| {
                b.iter(|| {
                    for index in 0..count {
                        black_box(storage.get::<Position>(index));
                    }
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    component_storage_benches,
    bench_push_fill,
    bench_pop_drain,
    bench_swap_remove_drain,
    bench_get_sweep,
);
criterion_main!(component_storage_benches);
