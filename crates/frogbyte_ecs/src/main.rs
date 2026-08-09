use frogbyte_ecs::entity::{Entity, entity_allocator::EntityAllocator};

pub fn main() {
    let mut entities: Vec<Entity> = Vec::new();
    let mut entity_allocator = EntityAllocator::new();
    for _ in 0..10 {
        entities.push(entity_allocator.create());
    }

    // Removing a live entity succeeds and frees its slot for reuse.
    println!("{:?}", entity_allocator.remove(entities[3]));
    // This reuses the slot just freed above, incrementing its generation.
    entity_allocator.create();
    // `entities[3]` is now a stale handle: it names the freed slot's index
    // but not its new generation, so this second removal is rejected.
    println!("{:?}", entity_allocator.remove(entities[3]));
}
