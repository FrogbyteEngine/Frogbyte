//! Benchmarks for [`EntityAllocator`] allocation, reuse, removal, churn, and
//! liveness-query paths.
//!
//! The batched workloads keep their allocator as Criterion batch state and
//! operate on it through a reference, so growing the slot storage is measured
//! while dropping it afterwards is not.
//!
//! Batch state scales with the entity count, so the populated workloads use
//! `BatchSize::LargeInput` to bound how much of it Criterion keeps alive at
//! once; only the single-slot churn workload has genuinely small state.
use std::hint::black_box;

use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use frogbyte_ecs::entity::{Entity, EntityAllocator};

/// Entity counts covering a small, medium, and large working set.
const ENTITY_COUNTS: [u32; 3] = [128, 1_024, 8_192];

/// Creates an allocator holding `count` live entities and returns it with the
/// handles that were issued.
///
/// Shared setup for the workloads that measure operations over an existing
/// population; it always runs outside the measured routine.
fn populated_allocator(count: u32) -> (EntityAllocator, Vec<Entity>) {
    let mut allocator = EntityAllocator::new();
    let entities: Vec<Entity> = (0..count).map(|_| allocator.create()).collect();

    (allocator, entities)
}

/// Creating entities into an allocator with no previously freed slots appends
/// a slot on every call, so this measures the growth path of the slot storage
/// including its amortized reallocations.
fn bench_create_fresh_slots(c: &mut Criterion) {
    let mut group = c.benchmark_group("entity_allocator_create_fresh");

    for &count in &ENTITY_COUNTS {
        group.throughput(Throughput::Elements(u64::from(count)));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{count}_entities")),
            &count,
            |b, &count| {
                b.iter_batched_ref(
                    EntityAllocator::new,
                    |allocator| {
                        for _ in 0..count {
                            black_box(allocator.create());
                        }
                    },
                    BatchSize::LargeInput,
                );
            },
        );
    }

    group.finish();
}

/// Creating entities when every slot has already been freed exercises the
/// slot-reuse path instead of growing the slot vector.
fn bench_create_reused_slots(c: &mut Criterion) {
    let mut group = c.benchmark_group("entity_allocator_create_reused");

    for &count in &ENTITY_COUNTS {
        group.throughput(Throughput::Elements(u64::from(count)));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{count}_entities")),
            &count,
            |b, &count| {
                b.iter_batched_ref(
                    || {
                        let (mut allocator, entities) = populated_allocator(count);

                        for entity in entities {
                            allocator
                                .remove(entity)
                                .expect("just-created entity should be alive");
                        }

                        allocator
                    },
                    |allocator| {
                        for _ in 0..count {
                            black_box(allocator.create());
                        }
                    },
                    BatchSize::LargeInput,
                );
            },
        );
    }

    group.finish();
}

/// Bulk removal of every live entity, as happens when a large group of
/// entities is despawned at once.
fn bench_remove_all(c: &mut Criterion) {
    let mut group = c.benchmark_group("entity_allocator_remove_all");

    for &count in &ENTITY_COUNTS {
        group.throughput(Throughput::Elements(u64::from(count)));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{count}_entities")),
            &count,
            |b, &count| {
                b.iter_batched_ref(
                    || populated_allocator(count),
                    |(allocator, entities)| {
                        // The handles are read out of the batch state instead
                        // of being consumed, so releasing that storage stays
                        // outside the measured removals.
                        for &entity in entities.iter() {
                            allocator
                                .remove(black_box(entity))
                                .expect("entity should be alive");
                        }
                    },
                    BatchSize::LargeInput,
                );
            },
        );
    }

    group.finish();
}

/// Scanning liveness across a mix of live and removed slots, which is the
/// read-only path a caller takes when validating stored handles in bulk.
fn bench_is_alive_scan(c: &mut Criterion) {
    let mut group = c.benchmark_group("entity_allocator_is_alive_scan");

    for &count in &ENTITY_COUNTS {
        let (mut allocator, entities) = populated_allocator(count);

        // Remove every other entity so the scan covers both live and dead
        // slots rather than an all-alive best case.
        for entity in entities.iter().step_by(2) {
            allocator
                .remove(*entity)
                .expect("entity should be alive before removal");
        }

        // `is_alive` only reads allocator state, so the same population is
        // reused across iterations instead of being rebuilt per iteration.
        let state = (allocator, entities);

        group.throughput(Throughput::Elements(u64::from(count)));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{count}_entities")),
            &state,
            |b, (allocator, entities)| {
                b.iter(|| {
                    for entity in entities {
                        black_box(allocator.is_alive(*entity));
                    }
                });
            },
        );
    }

    group.finish();
}

/// Repeated create/remove cycles on a single slot, matching a long-lived
/// object pool that is churned rather than a one-off bulk allocation.
///
/// The live population never exceeds one entity, so this reports the cost of
/// a free-list round trip over state that stays resident in cache. Compare it
/// with the steady-state churn workload below, which performs the same
/// operations over a population that grows with the entity count.
fn bench_create_remove_cycles(c: &mut Criterion) {
    let mut group = c.benchmark_group("entity_allocator_create_remove_cycles");

    for &count in &ENTITY_COUNTS {
        // One removal plus one creation per reported element.
        group.throughput(Throughput::Elements(u64::from(count)));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{count}_cycles")),
            &count,
            |b, &count| {
                b.iter_batched_ref(
                    || {
                        let mut allocator = EntityAllocator::new();
                        let entity = allocator.create();

                        (allocator, entity)
                    },
                    |(allocator, entity)| {
                        for _ in 0..count {
                            allocator
                                .remove(*entity)
                                .expect("entity should still be alive");
                            *entity = allocator.create();
                        }

                        black_box(*entity);
                    },
                    // A single slot and its handle are small enough to keep a
                    // full SmallInput batch of them alive.
                    BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

/// Steady-state churn, where each step removes a live entity and immediately
/// creates its replacement so the live population stays at `count` entities.
///
/// Unlike the single-slot cycles above, this walks a working set that grows
/// with the entity count, so it covers free-list reuse together with the
/// memory traffic of touching every slot.
fn bench_steady_state_churn(c: &mut Criterion) {
    let mut group = c.benchmark_group("entity_allocator_steady_state_churn");

    for &count in &ENTITY_COUNTS {
        // One removal plus one creation per reported element.
        group.throughput(Throughput::Elements(u64::from(count)));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{count}_entities")),
            &count,
            |b, &count| {
                b.iter_batched_ref(
                    || populated_allocator(count),
                    |(allocator, entities)| {
                        // Replacement handles are stored back so every step
                        // works from a current handle, as caller-side entity
                        // storage would have to.
                        for handle in entities.iter_mut() {
                            allocator
                                .remove(black_box(*handle))
                                .expect("entity should be alive");
                            *handle = allocator.create();
                        }
                    },
                    BatchSize::LargeInput,
                );
            },
        );
    }

    group.finish();
}

criterion_group!(
    entity_allocator_benches,
    bench_create_fresh_slots,
    bench_create_reused_slots,
    bench_remove_all,
    bench_is_alive_scan,
    bench_create_remove_cycles,
    bench_steady_state_churn,
);
criterion_main!(entity_allocator_benches);
