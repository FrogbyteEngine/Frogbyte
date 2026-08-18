use std::any::TypeId;

use frogbyte_ecs::{
    archetype::{Archetype, ArchetypeKey},
    component::Component,
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

#[test]
fn insert_and_get_single_component() {
    let mut archetype = Archetype::new::<(Position,)>();

    archetype.insert(Entity::new(0, 0), (Position { x: 10.0, y: 20.0 },));

    let position = archetype.get::<Position>(0);

    assert_eq!(position, &Position { x: 10.0, y: 20.0 });
}

#[test]
fn get_component_from_multiple_rows() {
    let mut archetype = Archetype::new::<(Position,)>();

    archetype.insert(Entity::new(0, 0), (Position { x: 1.0, y: 2.0 },));

    archetype.insert(Entity::new(1, 0), (Position { x: 3.0, y: 4.0 },));

    let first = archetype.get::<Position>(0);
    let second = archetype.get::<Position>(1);

    println!("first: {:?}", first);
    println!("second: {:?}", second);

    assert_eq!(first, &Position { x: 1.0, y: 2.0 });
    assert_eq!(second, &Position { x: 3.0, y: 4.0 });
}

#[test]
fn get_different_component_types_from_same_row() {
    let mut archetype = Archetype::new::<(Position, Velocity)>();

    archetype.insert(
        Entity::new(0, 0),
        (Position { x: 10.0, y: 20.0 }, Velocity { x: 1.0, y: 2.0 }),
    );

    let position = archetype.get::<Position>(0);
    let velocity = archetype.get::<Velocity>(0);

    assert_eq!(position, &Position { x: 10.0, y: 20.0 });

    assert_eq!(velocity, &Velocity { x: 1.0, y: 2.0 });
}

#[test]
fn component_tuple_order_does_not_matter() {
    let mut archetype = Archetype::new::<(Position, Velocity)>();

    archetype.insert(
        Entity::new(0, 0),
        (Velocity { x: 3.0, y: 4.0 }, Position { x: 1.0, y: 2.0 }),
    );

    assert_eq!(archetype.get::<Position>(0), &Position { x: 1.0, y: 2.0 });

    assert_eq!(archetype.get::<Velocity>(0), &Velocity { x: 3.0, y: 4.0 });
}

#[test]
fn multiple_components_and_multiple_rows_remain_aligned() {
    let mut archetype = Archetype::new::<(Position, Velocity, Health)>();

    archetype.insert(
        Entity::new(0, 0),
        (
            Position { x: 1.0, y: 2.0 },
            Velocity { x: 3.0, y: 4.0 },
            Health { value: 100 },
        ),
    );

    archetype.insert(
        Entity::new(1, 0),
        (
            Position { x: 5.0, y: 6.0 },
            Velocity { x: 7.0, y: 8.0 },
            Health { value: 50 },
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
#[should_panic(expected = "ComponentSet does not match Archetype")]
fn reject_component_set_that_does_not_match_archetype() {
    let mut archetype = Archetype::new::<(Position, Velocity)>();

    archetype.insert(
        Entity::new(0, 0),
        (Position { x: 1.0, y: 2.0 }, Health { value: 100 }),
    );
}

#[test]
#[should_panic(expected = "Archetype cannot contain duplicate component types")]
fn reject_duplicate_component_types() {
    let _archetype = Archetype::new::<(Position, Position)>();
}

#[test]
fn archetype_key_is_canonical() {
    let first = ArchetypeKey::new(vec![TypeId::of::<Position>(), TypeId::of::<Velocity>()]);

    let second = ArchetypeKey::new(vec![TypeId::of::<Velocity>(), TypeId::of::<Position>()]);

    assert_eq!(first, second);
}

#[test]
fn get_mut_updates_component() {
    let mut archetype = Archetype::new::<(Position,)>();

    archetype.insert(Entity::new(0, 0), (Position { x: 1.0, y: 2.0 },));

    {
        let position = archetype.get_mut::<Position>(0);
        position.x = 10.0;
        position.y = 20.0;
    }

    assert_eq!(archetype.get::<Position>(0), &Position { x: 10.0, y: 20.0 });
}

#[test]
fn empty_archetype_supports_entity_rows() {
    let first = Entity::new(0, 0);
    let second = Entity::new(1, 0);

    let mut archetype = Archetype::new::<()>();

    archetype.insert(first, ());
    archetype.insert(second, ());

    assert_eq!(archetype.swap_remove(0), Some(second));
    assert_eq!(archetype.swap_remove(0), None);
}

#[test]
fn swap_remove_moves_last_row_and_preserves_column_alignment() {
    let mut archetype = Archetype::new::<(Position, Velocity, Health)>();

    archetype.insert(
        Entity::new(0, 0),
        (
            Position { x: 1.0, y: 2.0 },
            Velocity { x: 3.0, y: 4.0 },
            Health { value: 100 },
        ),
    );

    archetype.insert(
        Entity::new(1, 0),
        (
            Position { x: 5.0, y: 6.0 },
            Velocity { x: 7.0, y: 8.0 },
            Health { value: 50 },
        ),
    );

    archetype.insert(
        Entity::new(2, 0),
        (
            Position { x: 9.0, y: 10.0 },
            Velocity { x: 11.0, y: 12.0 },
            Health { value: 25 },
        ),
    );

    let moved = archetype.swap_remove(1);

    assert_eq!(moved, Some(Entity::new(2, 0)));

    assert_eq!(archetype.get::<Position>(1), &Position { x: 9.0, y: 10.0 });
    assert_eq!(archetype.get::<Velocity>(1), &Velocity { x: 11.0, y: 12.0 });
    assert_eq!(archetype.get::<Health>(1), &Health { value: 25 });
}

#[test]
fn swap_remove_last_row_returns_none() {
    let mut archetype = Archetype::new::<(Position,)>();

    archetype.insert(Entity::new(0, 0), (Position { x: 1.0, y: 2.0 },));

    archetype.insert(Entity::new(1, 0), (Position { x: 3.0, y: 4.0 },));

    assert_eq!(archetype.swap_remove(1), None);

    assert_eq!(archetype.get::<Position>(0), &Position { x: 1.0, y: 2.0 });
}
