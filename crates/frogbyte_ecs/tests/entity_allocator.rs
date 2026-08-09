use frogbyte_ecs::entity::{Entity, EntityAllocator};

#[test]
fn create_assigns_sequential_indices_starting_at_zero() {
    let mut allocator = EntityAllocator::new();

    let first = allocator.create();
    let second = allocator.create();
    let third = allocator.create();

    assert_eq!(first.index(), 0);
    assert_eq!(second.index(), 1);
    assert_eq!(third.index(), 2);
}

#[test]
fn newly_created_entities_start_at_generation_zero() {
    let mut allocator = EntityAllocator::new();

    let entity = allocator.create();

    assert_eq!(entity.generation(), 0);
}

#[test]
fn default_allocator_behaves_like_new() {
    let mut allocator = EntityAllocator::default();

    let entity = allocator.create();

    assert_eq!(entity, Entity::new(0, 0));
}

#[test]
fn create_returns_unique_live_entities() {
    let mut allocator = EntityAllocator::new();

    let entities: Vec<Entity> = (0..16).map(|_| allocator.create()).collect();

    for (i, a) in entities.iter().enumerate() {
        for (j, b) in entities.iter().enumerate() {
            if i != j {
                assert_ne!(a, b, "distinct live entities must not compare equal");
            }
        }
    }
}

#[test]
fn remove_succeeds_for_a_live_entity() {
    let mut allocator = EntityAllocator::new();
    let entity = allocator.create();

    assert!(allocator.remove(entity).is_ok());
}

#[test]
fn remove_fails_for_an_entity_with_out_of_range_index() {
    let mut allocator = EntityAllocator::new();
    allocator.create();

    let unknown = Entity::new(42, 0);

    assert!(allocator.remove(unknown).is_err());
}

#[test]
fn remove_fails_for_an_entity_that_was_never_created() {
    let mut allocator = EntityAllocator::new();

    assert!(allocator.remove(Entity::new(0, 0)).is_err());
}

#[test]
fn remove_fails_when_the_same_entity_is_removed_twice() {
    let mut allocator = EntityAllocator::new();
    let entity = allocator.create();

    assert!(allocator.remove(entity).is_ok());
    assert!(allocator.remove(entity).is_err());
}

#[test]
fn removed_index_is_reused_with_an_incremented_generation() {
    let mut allocator = EntityAllocator::new();
    let first = allocator.create();

    allocator.remove(first).expect("first entity should be removed");

    let second = allocator.create();

    assert_eq!(second.index(), first.index());
    assert_eq!(second.generation(), first.generation() + 1);
}

#[test]
fn stale_handle_is_rejected_after_its_slot_is_reused() {
    let mut allocator = EntityAllocator::new();
    let first = allocator.create();

    allocator.remove(first).expect("first entity should be removed");

    let second = allocator.create();
    assert_ne!(first, second, "reused slot must not equal the stale handle");

    // The stale handle from before reuse must be rejected even though the
    // slot it points at is alive again under a new generation.
    assert!(allocator.remove(first).is_err());

    // The current, correctly generationed handle must still work.
    assert!(allocator.remove(second).is_ok());
}

#[test]
fn free_list_reuses_indices_in_last_removed_first_order() {
    let mut allocator = EntityAllocator::new();
    let a = allocator.create();
    let b = allocator.create();

    allocator.remove(a).expect("a should be removed");
    allocator.remove(b).expect("b should be removed");

    let reused_first = allocator.create();
    let reused_second = allocator.create();

    assert_eq!(reused_first.index(), b.index());
    assert_eq!(reused_second.index(), a.index());
}

#[test]
fn repeated_create_remove_cycles_keep_incrementing_generation() {
    let mut allocator = EntityAllocator::new();
    let mut previous = allocator.create();

    for cycle in 1..10 {
        allocator.remove(previous).expect("entity should still be alive");
        let next = allocator.create();

        assert_eq!(next.index(), previous.index());
        assert_eq!(next.generation(), cycle);

        previous = next;
    }
}

#[test]
fn entities_differing_only_in_generation_are_not_equal() {
    let a = Entity::new(0, 0);
    let b = Entity::new(0, 1);

    assert_ne!(a, b);
}

#[test]
fn entities_with_same_index_and_generation_are_equal() {
    let a = Entity::new(3, 2);
    let b = Entity::new(3, 2);

    assert_eq!(a, b);
}
