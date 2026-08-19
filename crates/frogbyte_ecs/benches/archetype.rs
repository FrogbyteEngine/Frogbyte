//! Benchmarks for archetype row operations.
//!
//! Archetype creation and population used as benchmark setup remain outside
//! measured removal and access operations.

use std::hint::black_box;

use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use frogbyte_ecs::{archetype::Archetype, component::Component, entity::Entity};

#[derive(Clone, Copy)]
struct Position([u64; 2]);

impl Component for Position {}

#[derive(Clone, Copy)]
struct Velocity([u64; 2]);

impl Component for Velocity {}

#[derive(Clone, Copy)]
struct Health([u64; 2]);

impl Component for Health {}

const ROW_COUNTS: [usize; 3] = [128, 1_024, 8_192];

fn row_at(index: usize) -> (Entity, Position, Velocity, Health) {
    let value = index as u64;

    (
        Entity::new(index as u32, 0),
        Position([value, value.wrapping_add(1)]),
        Velocity([value.wrapping_add(2), value.wrapping_add(3)]),
        Health([value.wrapping_add(4), value.wrapping_add(5)]),
    )
}

fn input_rows(count: usize) -> Vec<(Entity, Position, Velocity, Health)> {
    (0..count).map(row_at).collect()
}

fn populated_archetype(count: usize) -> Archetype {
    let mut archetype = Archetype::new::<(Position, Velocity, Health)>();

    for index in 0..count {
        let (entity, position, velocity, health) = row_at(index);

        archetype.insert(entity, (position, velocity, health));
    }

    archetype
}

fn bench_insert_rows(c: &mut Criterion) {
    let mut group = c.benchmark_group("archetype_insert");

    for &count in &ROW_COUNTS {
        group.throughput(Throughput::Elements(count as u64));

        group.bench_with_input(
            BenchmarkId::new("three_components", format!("{count}_rows")),
            &count,
            |b, &count| {
                b.iter_batched_ref(
                    || {
                        (
                            Archetype::new::<(Position, Velocity, Health)>(),
                            input_rows(count),
                        )
                    },
                    |(archetype, rows)| {
                        for &(entity, position, velocity, health) in rows.iter() {
                            archetype.insert(
                                black_box(entity),
                                (black_box(position), black_box(velocity), black_box(health)),
                            );
                        }
                    },
                    BatchSize::LargeInput,
                );
            },
        );
    }

    group.finish();
}

fn bench_get_sweep(c: &mut Criterion) {
    let mut group = c.benchmark_group("archetype_get_sweep");

    for &count in &ROW_COUNTS {
        let archetype = populated_archetype(count);

        group.throughput(Throughput::Elements((count * 3) as u64));

        group.bench_with_input(
            BenchmarkId::new("three_components", format!("{count}_rows")),
            &archetype,
            |b, archetype| {
                b.iter(|| {
                    for row in 0..count {
                        black_box(archetype.get::<Position>(row).0);
                        black_box(archetype.get::<Velocity>(row).0);
                        black_box(archetype.get::<Health>(row).0);
                    }
                });
            },
        );
    }

    group.finish();
}

fn bench_swap_remove_drain(c: &mut Criterion) {
    let mut group = c.benchmark_group("archetype_swap_remove");

    for &count in &ROW_COUNTS {
        group.throughput(Throughput::Elements(count as u64));

        group.bench_with_input(
            BenchmarkId::new("three_components", format!("{count}_rows")),
            &count,
            |b, &count| {
                b.iter_batched_ref(
                    || populated_archetype(count),
                    |archetype| {
                        for _ in 0..count {
                            black_box(archetype.swap_remove(0));
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
    archetype_benches,
    bench_insert_rows,
    bench_get_sweep,
    bench_swap_remove_drain,
);

criterion_main!(archetype_benches);
