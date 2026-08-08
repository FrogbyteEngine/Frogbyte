use frogbyte_ecs::entity::{entity::Entity, entity_allocator::EntityAllocator};

pub fn main() {
    let mut entities: Vec<Entity> = Vec::new();
    let mut entity_allocator = EntityAllocator::new();
    for _ in 0..10 {
        entities.push(entity_allocator.create());
    }

    println!("{:?}", entity_allocator.remove(entities[3]));
    entity_allocator.create();
    println!("{:?}", entity_allocator.remove(entities[3]));
}
