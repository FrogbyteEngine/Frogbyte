use frogbyte_ecs::entity::entity_allocator::EntityAllocator;

pub fn main() {
    let mut entity_allocator = EntityAllocator::new();
    for _ in 0..10 {
        entity_allocator.create();
    }

    println!("{:?}", entity_allocator.remove(3));
    println!("{:?}", entity_allocator.remove(16));
    entity_allocator.create();
    println!("{:?}", entity_allocator.remove(3));


}