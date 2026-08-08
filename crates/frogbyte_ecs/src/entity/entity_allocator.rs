#[derive(Debug)]
pub struct EntitySlot {
    generations: u32,
    is_alive: bool,
}

pub struct EntityAllocator {
    slots: Vec<EntitySlot>,
    free: Vec<u32>,
}

impl EntityAllocator {
    pub fn new() -> Self {
        Self {
            slots: Vec::with_capacity(32),
            free: Vec::new(),
        }
    }

    pub fn create(&mut self) {
        if let Some(index) = self.free.pop() {
            self.slots[index as usize].generations += 1;
            self.slots[index as usize].is_alive = true;
        } else {
            let slot = EntitySlot {
                generations: 0,
                is_alive: true,
            };

            self.slots.push(slot);
        }      
    }

    pub fn remove(&mut self, index: u32) -> Result<&EntitySlot, &str> {
        if let Some(slot) = self.slots.get_mut(index as usize) {
            self.free.push(index);
            slot.is_alive = false;
            return Ok(slot);
        }

        Err("Error: Can't remove an entity that does not exist.")
    }
}