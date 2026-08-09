//! Ownership and lifecycle management of entity slots.
use super::Entity;

/// The allocator's bookkeeping for a single entity slot.
///
/// The generation is incremented every time the slot is reused, which is
/// what allows a stale [`Entity`] handle to be distinguished from the
/// handle of whichever entity currently occupies the slot.
#[derive(Debug)]
struct EntitySlot {
    /// Incremented each time this slot is reused by [`EntityAllocator::create`].
    generation: u32,
    /// Whether the slot currently refers to a live entity.
    is_alive: bool,
}

/// Owns entity slots and creates and removes generational [`Entity`]
/// handles.
pub struct EntityAllocator {
    /// One entry per slot ever allocated; indexed by [`Entity::index`].
    slots: Vec<EntitySlot>,
    /// Indices of removed slots available for reuse, most-recently-freed
    /// last so [`Vec::pop`] returns the most recently freed slot first.
    ///
    /// This ordering is an implementation detail, not a documented API
    /// guarantee.
    free: Vec<u32>,
}

impl Default for EntityAllocator {
    fn default() -> Self {
        Self::new()
    }
}

impl EntityAllocator {
    /// Creates an empty allocator with no slots.
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            free: Vec::new(),
        }
    }

    /// Creates and returns a new, live entity.
    ///
    /// If a previously removed slot is available it is reused with its
    /// generation incremented, guaranteeing the returned handle differs
    /// from any handle previously issued for that slot. Otherwise a new
    /// slot is appended at generation `0`.
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

    /// Releases `entity`'s slot back to the allocator so a future
    /// [`EntityAllocator::create`] call may reuse it.
    ///
    /// Returns an error, without modifying allocator state, when `entity`
    /// does not currently refer to a live entity. This covers an index that
    /// was never allocated, an index whose slot is already dead, and a
    /// stale handle whose generation no longer matches the slot's current
    /// generation even though the slot is alive again under a new
    /// occupant.
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

    pub fn is_alive(&self, entity: Entity) -> bool {
        if let Some(slot) = self.slots.get(entity.index() as usize) {
            return slot.is_alive;
        }

        false
    }
}
