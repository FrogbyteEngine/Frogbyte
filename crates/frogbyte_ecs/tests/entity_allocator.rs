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

    allocator
        .remove(first)
        .expect("first entity should be removed");

    let second = allocator.create();

    assert_eq!(second.index(), first.index());
    assert_eq!(second.generation(), first.generation() + 1);
}

#[test]
fn stale_handle_is_rejected_after_its_slot_is_reused() {
    let mut allocator = EntityAllocator::new();
    let first = allocator.create();

    allocator
        .remove(first)
        .expect("first entity should be removed");

    let second = allocator.create();
    assert_ne!(first, second, "reused slot must not equal the stale handle");

    // The stale handle from before reuse must be rejected even though the
    // slot it points at is alive again under a new generation.
    assert!(allocator.remove(first).is_err());

    // The current, correctly generationed handle must still work.
    assert!(allocator.remove(second).is_ok());
}

#[test]
fn repeated_create_remove_cycles_keep_incrementing_generation() {
    let mut allocator = EntityAllocator::new();
    let mut previous = allocator.create();

    for cycle in 1..10 {
        allocator
            .remove(previous)
            .expect("entity should still be alive");
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

#[test]
fn is_alive_is_false_for_an_index_that_was_never_allocated() {
    let allocator = EntityAllocator::new();

    assert!(!allocator.is_alive(Entity::new(0, 0)));
}

#[test]
fn is_alive_is_false_for_an_out_of_range_index() {
    let mut allocator = EntityAllocator::new();
    allocator.create();

    assert!(!allocator.is_alive(Entity::new(42, 0)));
}

#[test]
fn is_alive_is_true_for_a_freshly_created_entity() {
    let mut allocator = EntityAllocator::new();

    let entity = allocator.create();

    assert!(allocator.is_alive(entity));
}

#[test]
fn is_alive_is_false_after_removal() {
    let mut allocator = EntityAllocator::new();
    let entity = allocator.create();

    allocator.remove(entity).expect("entity should be alive");

    assert!(!allocator.is_alive(entity));
}

#[test]
fn is_alive_tracks_each_slot_independently() {
    let mut allocator = EntityAllocator::new();
    let first = allocator.create();
    let second = allocator.create();
    let third = allocator.create();

    allocator
        .remove(second)
        .expect("second entity should be alive");

    assert!(allocator.is_alive(first), "untouched slot must stay alive");
    assert!(
        !allocator.is_alive(second),
        "removed slot must not be alive"
    );
    assert!(allocator.is_alive(third), "untouched slot must stay alive");
}

#[test]
fn is_alive_is_false_for_a_stale_handle_after_its_slot_is_reused() {
    let mut allocator = EntityAllocator::new();
    let first = allocator.create();

    allocator
        .remove(first)
        .expect("first entity should be removed");

    let second = allocator.create();
    assert_ne!(first, second, "reused slot must not equal the stale handle");

    // The new occupant is alive, but the stale handle that pointed at the
    // same index before reuse must not be reported as alive: liveness is a
    // property of the exact (index, generation) handle, not of the index
    // alone, matching the generation check `remove` already performs.
    assert!(allocator.is_alive(second));
    assert!(
        !allocator.is_alive(first),
        "a stale handle must not be reported as alive after its slot is reused"
    );
}

#[test]
fn is_alive_reflects_repeated_create_remove_cycles() {
    let mut allocator = EntityAllocator::new();
    let mut previous = allocator.create();

    for _ in 1..10 {
        assert!(allocator.is_alive(previous));

        allocator
            .remove(previous)
            .expect("entity should still be alive");
        assert!(!allocator.is_alive(previous));

        previous = allocator.create();
    }
}
