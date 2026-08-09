//! Benchmarks for [`EntityAllocator`] allocation, reuse, removal, and
//! liveness-query paths.
use std::hint::black_box;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use frogbyte_ecs::entity::EntityAllocator;

/// Entity counts covering a small, medium, and large working set.
const ENTITY_COUNTS: [u32; 3] = [128, 1_024, 8_192];

/// Creating entities into an allocator with no previously freed slots grows
/// the slot vector on every call; this is the allocator's worst-case
/// allocation path.
fn bench_create_fresh_slots(c: &mut Criterion) {
    let mut group = c.benchmark_group("entity_allocator_create_fresh");

    for &count in &ENTITY_COUNTS {
        group.bench_function(format!("{count}_entities"), |b| {
            b.iter_batched(
                EntityAllocator::new,
                |mut allocator| {
                    for _ in 0..count {
                        black_box(allocator.create());
                    }
                },
                BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

/// Creating entities when every slot has already been freed exercises the
/// slot-reuse path instead of growing the slot vector.
fn bench_create_reused_slots(c: &mut Criterion) {
    let mut group = c.benchmark_group("entity_allocator_create_reused");

    for &count in &ENTITY_COUNTS {
        group.bench_function(format!("{count}_entities"), |b| {
            b.iter_batched(
                || {
                    let mut allocator = EntityAllocator::new();
                    let entities: Vec<_> = (0..count).map(|_| allocator.create()).collect();
                    for entity in entities {
                        allocator
                            .remove(entity)
                            .expect("just-created entity should be alive");
                    }
                    allocator
                },
                |mut allocator| {
                    for _ in 0..count {
                        black_box(allocator.create());
                    }
                },
                BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

/// Bulk removal of every live entity, as happens when a large group of
/// entities is despawned at once.
fn bench_remove_all(c: &mut Criterion) {
    let mut group = c.benchmark_group("entity_allocator_remove_all");

    for &count in &ENTITY_COUNTS {
        group.bench_function(format!("{count}_entities"), |b| {
            b.iter_batched(
                || {
                    let mut allocator = EntityAllocator::new();
                    let entities: Vec<_> = (0..count).map(|_| allocator.create()).collect();
                    (allocator, entities)
                },
                |(mut allocator, entities)| {
                    for entity in entities {
                        allocator
                            .remove(black_box(entity))
                            .expect("entity should be alive");
                    }
                },
                BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

/// Scanning liveness across a mix of live and removed slots, as a query
/// would when filtering out despawned entities.
fn bench_is_alive_scan(c: &mut Criterion) {
    let mut group = c.benchmark_group("entity_allocator_is_alive_scan");

    for &count in &ENTITY_COUNTS {
        let mut allocator = EntityAllocator::new();
        let entities: Vec<_> = (0..count).map(|_| allocator.create()).collect();

        // Remove every other entity so the scan covers both live and dead
        // slots rather than an all-alive best case.
        for entity in entities.iter().step_by(2) {
            allocator
                .remove(*entity)
                .expect("entity should be alive before removal");
        }

        group.bench_function(format!("{count}_entities"), |b| {
            b.iter(|| {
                for entity in &entities {
                    black_box(allocator.is_alive(*entity));
                }
            });
        });
    }

    group.finish();
}

/// Repeated create/remove cycles on a single slot, matching a long-lived
/// object pool that is churned rather than a one-off bulk allocation.
fn bench_create_remove_cycles(c: &mut Criterion) {
    let mut group = c.benchmark_group("entity_allocator_create_remove_cycles");

    for &count in &ENTITY_COUNTS {
        group.bench_function(format!("{count}_cycles"), |b| {
            b.iter_batched(
                || {
                    let mut allocator = EntityAllocator::new();
                    let entity = allocator.create();
                    (allocator, entity)
                },
                |(mut allocator, mut entity)| {
                    for _ in 0..count {
                        allocator
                            .remove(entity)
                            .expect("entity should still be alive");
                        entity = allocator.create();
                    }
                    black_box(entity);
                },
                BatchSize::SmallInput,
            );
        });
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
);
criterion_main!(entity_allocator_benches);
