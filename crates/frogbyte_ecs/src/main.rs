//! Manual smoke-test entry point for entity allocation, removal, and
//! generation-based staleness detection.
use frogbyte_ecs::entity::{Entity, entity_allocator::EntityAllocator};

pub fn main() {
    let mut entities: Vec<Entity> = Vec::new();
    let mut entity_allocator = EntityAllocator::new();
    for _ in 0..10 {
        entities.push(entity_allocator.create());
    }

    // Removing a live entity succeeds.
    println!("{:?}", entity_allocator.remove(entities[3]));
    // Reusing the freed slot increments its generation.
    entity_allocator.create();
    // `entities[3]` is now a stale handle for the reused slot, so removing
    // it again is expected to fail.
    println!("{:?}", entity_allocator.remove(entities[3]));
}
