use std::{
    any::TypeId,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

use frogbyte_ecs::{
    archetype::{Archetype, ArchetypeKey},
    component::{Component, component_set::ComponentSet},
    entity::Entity,
};

#[derive(Debug, PartialEq)]
struct Position {
    x: f32,
    y: f32,
}

impl Component for Position {}

#[derive(Debug, PartialEq)]
struct Velocity {
    x: f32,
    y: f32,
}

impl Component for Velocity {}

#[derive(Debug, PartialEq)]
struct Health {
    value: u32,
}

impl Component for Health {}

#[derive(Debug, PartialEq)]
struct Marker;

impl Component for Marker {}

struct TagA;

impl Component for TagA {}

struct TagB;

impl Component for TagB {}

struct TrackedComponent {
    id: u32,
    drops: Arc<AtomicUsize>,
}

impl Component for TrackedComponent {}

impl Drop for TrackedComponent {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

struct PanicOnDrop {
    id: u32,
    drop_attempts: Arc<AtomicUsize>,
    panic_once: Arc<AtomicBool>,
}

impl Component for PanicOnDrop {}

impl Drop for PanicOnDrop {
    fn drop(&mut self) {
        self.drop_attempts.fetch_add(1, Ordering::SeqCst);

        if self.panic_once.swap(false, Ordering::SeqCst) {
            panic!("intentional component drop panic");
        }
    }
}

struct DropProbe {
    id: u32,
    drops: Arc<AtomicUsize>,
}

impl Component for DropProbe {}

impl Drop for DropProbe {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn archetype_key_is_canonical() {
    let first = ArchetypeKey::new(vec![
        TypeId::of::<Position>(),
        TypeId::of::<Velocity>(),
        TypeId::of::<Health>(),
    ]);

    let second = ArchetypeKey::new(vec![
        TypeId::of::<Health>(),
        TypeId::of::<Position>(),
        TypeId::of::<Velocity>(),
    ]);

    assert_eq!(first, second);
}

#[test]
#[should_panic(expected = "Archetype cannot contain duplicate component types")]
fn archetype_rejects_duplicate_component_types() {
    let _archetype = Archetype::new::<(Position, Position)>();
}

#[test]
fn supported_tuples_implement_component_set() {
    fn assert_component_set<T: ComponentSet>() {}

    assert_component_set::<()>();
    assert_component_set::<(Position,)>();
    assert_component_set::<(Position, Velocity, Health)>();
    assert_component_set::<(Position, Velocity, Health, Marker, TagA, TagB)>();
}

#[test]
fn insert_routes_components_by_type_and_preserves_rows() {
    let mut archetype = Archetype::new::<(Position, Velocity, Health)>();

    archetype.insert(
        Entity::new(0, 0),
        (
            Health { value: 100 },
            Position { x: 1.0, y: 2.0 },
            Velocity { x: 3.0, y: 4.0 },
        ),
    );

    archetype.insert(
        Entity::new(1, 0),
        (
            Velocity { x: 7.0, y: 8.0 },
            Health { value: 50 },
            Position { x: 5.0, y: 6.0 },
        ),
    );

    assert_eq!(archetype.get::<Position>(0), &Position { x: 1.0, y: 2.0 });
    assert_eq!(archetype.get::<Velocity>(0), &Velocity { x: 3.0, y: 4.0 });
    assert_eq!(archetype.get::<Health>(0), &Health { value: 100 });

    assert_eq!(archetype.get::<Position>(1), &Position { x: 5.0, y: 6.0 });
    assert_eq!(archetype.get::<Velocity>(1), &Velocity { x: 7.0, y: 8.0 });
    assert_eq!(archetype.get::<Health>(1), &Health { value: 50 });
}

#[test]
fn get_mut_updates_only_requested_component() {
    let mut archetype = Archetype::new::<(Position, Velocity)>();

    archetype.insert(
        Entity::new(0, 0),
        (Position { x: 1.0, y: 2.0 }, Velocity { x: 3.0, y: 4.0 }),
    );

    {
        let position = archetype.get_mut::<Position>(0);
        position.x = 10.0;
        position.y = 20.0;
    }

    assert_eq!(archetype.get::<Position>(0), &Position { x: 10.0, y: 20.0 });

    assert_eq!(archetype.get::<Velocity>(0), &Velocity { x: 3.0, y: 4.0 });
}

#[test]
fn mismatched_component_set_is_rejected_before_insertion() {
    let mut archetype = Archetype::new::<(Position, Velocity)>();

    let result = catch_unwind(AssertUnwindSafe(|| {
        archetype.insert(
            Entity::new(0, 0),
            (Position { x: 1.0, y: 2.0 }, Health { value: 100 }),
        );
    }));

    assert!(result.is_err());

    // A valid insertion must still become row 0, proving that the rejected
    // insertion did not append a partial row.
    archetype.insert(
        Entity::new(1, 0),
        (Position { x: 5.0, y: 6.0 }, Velocity { x: 7.0, y: 8.0 }),
    );

    assert_eq!(archetype.get::<Position>(0), &Position { x: 5.0, y: 6.0 });
    assert_eq!(archetype.get::<Velocity>(0), &Velocity { x: 7.0, y: 8.0 });
}

#[test]
fn empty_archetype_supports_entity_rows() {
    let first = Entity::new(0, 0);
    let second = Entity::new(1, 0);
    let third = Entity::new(2, 0);

    let mut archetype = Archetype::new::<()>();

    archetype.insert(first, ());
    archetype.insert(second, ());
    archetype.insert(third, ());

    // Removing the middle row moves the final entity into its place.
    assert_eq!(archetype.swap_remove(1), Some(third));

    // The moved row is now the final row.
    assert_eq!(archetype.swap_remove(1), None);

    // Only the first entity remains.
    assert_eq!(archetype.swap_remove(0), None);
}

#[test]
fn swap_remove_moves_last_row_and_preserves_component_alignment() {
    let first = Entity::new(0, 0);
    let removed = Entity::new(1, 0);
    let moved = Entity::new(2, 0);

    let mut archetype = Archetype::new::<(Position, Velocity, Health)>();

    archetype.insert(
        first,
        (
            Position { x: 1.0, y: 2.0 },
            Velocity { x: 3.0, y: 4.0 },
            Health { value: 100 },
        ),
    );

    archetype.insert(
        removed,
        (
            Position { x: 5.0, y: 6.0 },
            Velocity { x: 7.0, y: 8.0 },
            Health { value: 50 },
        ),
    );

    archetype.insert(
        moved,
        (
            Position { x: 9.0, y: 10.0 },
            Velocity { x: 11.0, y: 12.0 },
            Health { value: 25 },
        ),
    );

    assert_eq!(archetype.swap_remove(1), Some(moved));

    // Row 0 remains untouched.
    assert_eq!(archetype.get::<Position>(0), &Position { x: 1.0, y: 2.0 });
    assert_eq!(archetype.get::<Velocity>(0), &Velocity { x: 3.0, y: 4.0 });
    assert_eq!(archetype.get::<Health>(0), &Health { value: 100 });

    // The previous final row has moved into row 1 as one aligned unit.
    assert_eq!(archetype.get::<Position>(1), &Position { x: 9.0, y: 10.0 });
    assert_eq!(archetype.get::<Velocity>(1), &Velocity { x: 11.0, y: 12.0 });
    assert_eq!(archetype.get::<Health>(1), &Health { value: 25 });
}

#[test]
fn swap_remove_last_row_does_not_move_an_entity() {
    let mut archetype = Archetype::new::<(Position,)>();

    archetype.insert(Entity::new(0, 0), (Position { x: 1.0, y: 2.0 },));
    archetype.insert(Entity::new(1, 0), (Position { x: 3.0, y: 4.0 },));

    assert_eq!(archetype.swap_remove(1), None);

    assert_eq!(archetype.get::<Position>(0), &Position { x: 1.0, y: 2.0 });
}

#[test]
fn swap_remove_drops_removed_value_exactly_once() {
    let retained_drops = Arc::new(AtomicUsize::new(0));
    let removed_drops = Arc::new(AtomicUsize::new(0));
    let moved_drops = Arc::new(AtomicUsize::new(0));

    let mut archetype = Archetype::new::<(TrackedComponent,)>();

    archetype.insert(
        Entity::new(0, 0),
        (TrackedComponent {
            id: 0,
            drops: Arc::clone(&retained_drops),
        },),
    );

    archetype.insert(
        Entity::new(1, 0),
        (TrackedComponent {
            id: 1,
            drops: Arc::clone(&removed_drops),
        },),
    );

    archetype.insert(
        Entity::new(2, 0),
        (TrackedComponent {
            id: 2,
            drops: Arc::clone(&moved_drops),
        },),
    );

    assert_eq!(archetype.swap_remove(1), Some(Entity::new(2, 0)));

    // Only the removed logical value must be destroyed by swap_remove.
    assert_eq!(removed_drops.load(Ordering::SeqCst), 1);
    assert_eq!(retained_drops.load(Ordering::SeqCst), 0);
    assert_eq!(moved_drops.load(Ordering::SeqCst), 0);

    // The retained and moved values are still alive in the archetype.
    assert_eq!(archetype.get::<TrackedComponent>(0).id, 0);
    assert_eq!(archetype.get::<TrackedComponent>(1).id, 2);

    drop(archetype);

    // Every logical value has now been destroyed exactly once.
    assert_eq!(removed_drops.load(Ordering::SeqCst), 1);
    assert_eq!(retained_drops.load(Ordering::SeqCst), 1);
    assert_eq!(moved_drops.load(Ordering::SeqCst), 1);
}

#[test]
fn swap_remove_finishes_pending_drops_when_a_destructor_panics() {
    let removed_panic_attempts = Arc::new(AtomicUsize::new(0));
    let moved_panic_attempts = Arc::new(AtomicUsize::new(0));

    let removed_probe_drops = Arc::new(AtomicUsize::new(0));
    let moved_probe_drops = Arc::new(AtomicUsize::new(0));

    let removed_should_panic = Arc::new(AtomicBool::new(true));
    let moved_should_panic = Arc::new(AtomicBool::new(false));

    let mut archetype = Archetype::new::<(PanicOnDrop, DropProbe)>();

    archetype.insert(
        Entity::new(0, 0),
        (
            PanicOnDrop {
                id: 0,
                drop_attempts: Arc::clone(&removed_panic_attempts),
                panic_once: Arc::clone(&removed_should_panic),
            },
            DropProbe {
                id: 0,
                drops: Arc::clone(&removed_probe_drops),
            },
        ),
    );

    archetype.insert(
        Entity::new(1, 0),
        (
            PanicOnDrop {
                id: 1,
                drop_attempts: Arc::clone(&moved_panic_attempts),
                panic_once: Arc::clone(&moved_should_panic),
            },
            DropProbe {
                id: 1,
                drops: Arc::clone(&moved_probe_drops),
            },
        ),
    );

    let result = catch_unwind(AssertUnwindSafe(|| {
        archetype.swap_remove(0);
    }));

    assert!(result.is_err());

    // The panicking destructor was attempted exactly once.
    assert_eq!(removed_panic_attempts.load(Ordering::SeqCst), 1);

    // The other removed component must also have been destroyed exactly once.
    // This assertion is independent of TypeId column ordering: DropProbe may
    // have been processed either before the panic or by DropGuard while
    // unwinding.
    assert_eq!(removed_probe_drops.load(Ordering::SeqCst), 1);

    // Structural removal completed before component destruction began, so the
    // previous final row is still aligned at row 0 after catching the panic.
    assert_eq!(archetype.get::<PanicOnDrop>(0).id, 1);
    assert_eq!(archetype.get::<DropProbe>(0).id, 1);

    // The moved values are still alive.
    assert_eq!(moved_panic_attempts.load(Ordering::SeqCst), 0);
    assert_eq!(moved_probe_drops.load(Ordering::SeqCst), 0);

    drop(archetype);

    // The moved values are destroyed normally when the archetype is dropped.
    assert_eq!(moved_panic_attempts.load(Ordering::SeqCst), 1);
    assert_eq!(moved_probe_drops.load(Ordering::SeqCst), 1);
}

#[test]
fn zero_sized_components_support_swap_remove() {
    let mut archetype = Archetype::new::<(Marker,)>();

    archetype.insert(Entity::new(0, 0), (Marker,));
    archetype.insert(Entity::new(1, 0), (Marker,));
    archetype.insert(Entity::new(2, 0), (Marker,));

    assert_eq!(archetype.swap_remove(0), Some(Entity::new(2, 0)));

    assert_eq!(archetype.get::<Marker>(0), &Marker);
    assert_eq!(archetype.get::<Marker>(1), &Marker);
}
