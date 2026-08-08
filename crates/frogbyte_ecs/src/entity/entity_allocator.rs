use std::collections::BTreeSet;

use crate::entity::entity::Entity;

#[derive(Debug)]
struct EntitySlot {
    generations: u32,
    is_alive: bool,
}

pub struct EntityAllocator {
    slots: Vec<EntitySlot>,
    free: BTreeSet<u32>,
}

impl EntityAllocator {
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            free: BTreeSet::new(),
        }
    }

    pub fn create(&mut self) -> Entity {
        if let Some(index) = self.free.pop_last() {
            let slot = &mut self.slots[index as usize];
            slot.generations += 1;
            slot.is_alive = true;

            return Entity::new(index, slot.generations);
            
        }

        let slot = EntitySlot {
            generations: 0,
            is_alive: true,
        };

        self.slots.push(slot);
        
        Entity::new(self.slots.len() as u32, 0)
    }

    pub fn remove(&mut self, entity: Entity) -> Result<(), &str> {
        if let Some(slot) = self.slots.get_mut(entity.index() as usize) {
            self.free.insert(entity.index());
            slot.is_alive = false;
            return Ok(());
        }

        Err("Error: Can't remove an entity that does not exist.")
    }
}