use crate::entity::entity::Entity;

#[derive(Debug)]
struct EntitySlot {
    generation: u32,
    is_alive: bool,
}

pub struct EntityAllocator {
    slots: Vec<EntitySlot>,
    free: Vec<u32>,
}

impl Default for EntityAllocator {
    fn default() -> Self {
        Self::new()
    }
}

impl EntityAllocator {
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            free: Vec::new(),
        }
    }

    pub fn create(&mut self) -> Entity {
        if let Some(index) = self.free.pop() {
            let slot = &mut self.slots[index as usize];
            slot.generation += 1;
            slot.is_alive = true;

            return Entity::new(index, slot.generation);
        }

        let slot = EntitySlot {
            generation: 0,
            is_alive: true,
        };

        self.slots.push(slot);

        Entity::new((self.slots.len() - 1) as u32, 0)
    }

    pub fn remove(&mut self, entity: Entity) -> Result<(), &str> {
        if let Some(slot) = self.slots.get_mut(entity.index() as usize)
            && slot.is_alive
            && slot.generation == entity.generation()
        {
            self.free.push(entity.index());
            slot.is_alive = false;
            return Ok(());
        }

        Err("Error: Can't remove an entity that does not exist.")
    }
}
