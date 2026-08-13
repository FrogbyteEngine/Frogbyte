//! Benchmarks for the type-erased component storage and the generic storage it
//! replaces.
//!
//! Insertion, removal, and release are measured for both `BlobVec` and the
//! generic storage kept under `support::generic_storage_baseline`, so those
//! groups expose a `blobvec` and a `generic_baseline` id over the same
//! component counts. The two ids of a group run the same workload from the
//! same logical state, so their difference reports what the current storage
//! costs relative to the one it replaced. That difference is not attributable
//! to type erasure alone: the two implementations also grow their buffer and
//! move components differently.
//!
//! Element access is measured for `BlobVec` only, because the retained baseline
//! exposes no accessor API.
//!
//! Most mutating workloads keep their storage as Criterion batch state and
//! operate on it through a reference, so populating a storage is measured only
//! where that is the workload, while releasing it afterwards is not. The
//! release group is the deliberate exception: it takes the storage by value so
//! that destruction is the measured operation.
//!
//! Batch state scales with the component count, so the populated workloads use
//! `BatchSize::LargeInput` to bound how much of it Criterion keeps alive at
//! once.
mod support;

use std::hint::black_box;

use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use frogbyte_ecs::component::blobvec::BlobVec;
use frogbyte_ecs::component::position_component::Position;

use crate::support::generic_storage_baseline::GenericStorageBaseline;

/// Component counts covering a small, medium, and large storage.
///
/// `Position` holds three `f32` fields, so the largest count keeps about 96 KiB
/// of component data and the scanning workloads reach past a typical L1 data
/// cache instead of only reporting a resident working set.
const COMPONENT_COUNTS: [usize; 3] = [128, 1_024, 8_192];

/// Builds a component whose fields vary with `index`, so a storage never holds
/// a single repeated value.
fn position_at(index: usize) -> Position {
    let base = index as f32;

    Position {
        x: base,
        y: base + 1.0,
        z: base + 2.0,
    }
}

/// Creates a type-erased storage holding `count` components.
///
/// Shared setup for the workloads that operate on an existing storage; it
/// always runs outside the measured routine.
fn populated_blob_vec(count: usize) -> BlobVec {
    let mut storage = BlobVec::new::<Position>();

    for index in 0..count {
        storage.push(position_at(index));
    }

    storage
}

/// Baseline counterpart of `populated_blob_vec`, holding the same components.
fn populated_baseline(count: usize) -> GenericStorageBaseline<Position> {
    let mut storage = GenericStorageBaseline::new();

    for index in 0..count {
        storage.push(position_at(index));
    }

    storage
}

/// Filling an empty storage, as when components are inserted for a batch of
/// newly created entities.
///
/// Both implementations start empty, so the measurement covers each growth
/// strategy and its reallocations together with the per-component write.
///
/// The baseline buffer is allocated when the storage is constructed, so that
/// first allocation falls into setup, while `BlobVec` allocates lazily on the
/// first insertion and pays for it inside the measurement. The gap is one
/// allocation out of the growth sequence and matters most at the smallest
/// count.
fn bench_push(c: &mut Criterion) {
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
                            // The index is hidden from the optimizer so the
                            // inserted components cannot be folded into a
                            // constant fill.
                            storage.push(position_at(black_box(index)));
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
                            storage.push(position_at(black_box(index)));
                        }
                    },
                    BatchSize::LargeInput,
                );
            },
        );
    }

    group.finish();
}

/// Draining a storage from the end, which is the removal path taken when the
/// trailing components are released one after another.
///
/// It uses the same population and element count as the swap-removal workload
/// below, so comparing the two groups shows what removing a component from an
/// arbitrary index costs relative to removing the trailing one.
fn bench_pop_all(c: &mut Criterion) {
    let mut group = c.benchmark_group("component_storage_pop_all");

    for &count in &COMPONENT_COUNTS {
        group.throughput(Throughput::Elements(count as u64));

        group.bench_with_input(
            BenchmarkId::new("blobvec", format!("{count}_components")),
            &count,
            |b, &count| {
                b.iter_batched_ref(
                    || populated_blob_vec(count),
                    |storage| {
                        for _ in 0..count {
                            // `Position` has no destructor, so discarding the
                            // returned component adds no teardown work to the
                            // measured removals.
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

/// Bulk removal that keeps the storage dense, as when a large group of
/// components is released without leaving holes behind.
///
/// Removing index 0 every time means each removal also moves the current last
/// component into the freed slot, so the measurement covers that move and the
/// length bookkeeping rather than removal from the end alone.
fn bench_swap_remove_all(c: &mut Criterion) {
    let mut group = c.benchmark_group("component_storage_swap_remove_all");

    for &count in &COMPONENT_COUNTS {
        group.throughput(Throughput::Elements(count as u64));

        group.bench_with_input(
            BenchmarkId::new("blobvec", format!("{count}_components")),
            &count,
            |b, &count| {
                b.iter_batched_ref(
                    || populated_blob_vec(count),
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

/// Steady-state churn, where every step removes one component and immediately
/// inserts a replacement so the stored component count stays at `count`.
///
/// The removal index walks the whole storage, so this covers a removal, the
/// component move it triggers, and the following insertion over a working set
/// that grows with the component count. The setup capacity already covers
/// `count` components, so no reallocation happens inside the measurement.
fn bench_steady_state_churn(c: &mut Criterion) {
    let mut group = c.benchmark_group("component_storage_steady_state_churn");

    for &count in &COMPONENT_COUNTS {
        // One removal plus one insertion per reported element.
        group.throughput(Throughput::Elements(count as u64));

        group.bench_with_input(
            BenchmarkId::new("blobvec", format!("{count}_components")),
            &count,
            |b, &count| {
                b.iter_batched_ref(
                    || populated_blob_vec(count),
                    |storage| {
                        for index in 0..count {
                            // Each insertion restores the length to `count`, so
                            // the walking removal index stays in bounds.
                            black_box(storage.swap_remove::<Position>(index));
                            storage.push(position_at(black_box(index)));
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
                        for index in 0..count {
                            black_box(storage.swap_remove(index));
                            storage.push(position_at(black_box(index)));
                        }
                    },
                    BatchSize::LargeInput,
                );
            },
        );
    }

    group.finish();
}

/// Reading every stored component in index order, the sequential access the
/// contiguous layout exists for.
///
/// Access is exposed one component at a time, so the reported per-component
/// cost includes the bounds check and the stored-type check that `get`
/// performs on every call.
fn bench_sequential_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("component_storage_sequential_read");

    for &count in &COMPONENT_COUNTS {
        // Reading leaves the storage unchanged, so a single population is
        // shared by every iteration instead of being rebuilt.
        let storage = populated_blob_vec(count);

        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(
            BenchmarkId::new("blobvec", format!("{count}_components")),
            &storage,
            |b, storage| {
                b.iter(|| {
                    for index in 0..count {
                        let position = storage
                            .get::<Position>(index)
                            .expect("index below the stored component count");

                        // Every field is consumed so the whole component is
                        // loaded rather than only its first field.
                        black_box(position.x + position.y + position.z);
                    }
                });
            },
        );
    }

    group.finish();
}

/// Updating every stored component in index order, the write counterpart of the
/// sequential read.
fn bench_sequential_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("component_storage_sequential_write");

    for &count in &COMPONENT_COUNTS {
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(
            BenchmarkId::new("blobvec", format!("{count}_components")),
            &count,
            |b, &count| {
                b.iter_batched_ref(
                    || populated_blob_vec(count),
                    |storage| {
                        for index in 0..count {
                            let position = storage
                                .get_mut::<Position>(index)
                                .expect("index below the stored component count");

                            // Negating every field reads and writes the whole
                            // component while keeping the stored values bounded
                            // however often the storage is updated.
                            position.x = -position.x;
                            position.y = -position.y;
                            position.z = -position.z;
                        }

                        // The updated storage has to be observed, otherwise the
                        // writes above could be treated as dead.
                        black_box(storage);
                    },
                    BatchSize::LargeInput,
                );
            },
        );
    }

    group.finish();
}

/// Releasing a populated storage, as when a component storage is discarded
/// whole instead of being emptied one component at a time.
///
/// Population happens in setup and the storage is taken by value, so the
/// measurement covers destroying every stored component plus the single
/// deallocation. This is the one lifecycle stage the other groups deliberately
/// keep out of their measurement, and it scales with the component count:
/// `BlobVec` stores the drop function chosen at construction and calls it once
/// per stored component, whichever component type it holds. The baseline
/// destroys the same components through a statically known type, so the two ids
/// bracket what erasing the component type costs on release. `Position` has no
/// destructor, which is the case where the two release paths have the most room
/// to differ.
fn bench_drop_populated(c: &mut Criterion) {
    let mut group = c.benchmark_group("component_storage_drop");

    for &count in &COMPONENT_COUNTS {
        group.throughput(Throughput::Elements(count as u64));

        group.bench_with_input(
            BenchmarkId::new("blobvec", format!("{count}_components")),
            &count,
            |b, &count| {
                // The routine consumes the storage, so its destructor runs
                // inside the measured region. Returning the storage instead
                // would move that work after the timer stops, because Criterion
                // releases batch outputs outside the measurement.
                b.iter_batched(
                    || populated_blob_vec(count),
                    drop::<BlobVec>,
                    BatchSize::LargeInput,
                );
            },
        );

        group.bench_with_input(
            BenchmarkId::new("generic_baseline", format!("{count}_components")),
            &count,
            |b, &count| {
                b.iter_batched(
                    || populated_baseline(count),
                    drop::<GenericStorageBaseline<Position>>,
                    BatchSize::LargeInput,
                );
            },
        );
    }

    group.finish();
}

criterion_group!(
    component_storage_benches,
    bench_push,
    bench_pop_all,
    bench_swap_remove_all,
    bench_steady_state_churn,
    bench_sequential_read,
    bench_sequential_write,
    bench_drop_populated,
);
criterion_main!(component_storage_benches);
