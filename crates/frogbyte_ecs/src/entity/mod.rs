//! Generational entity identity.
//!
//! An [`Entity`] is a lightweight handle made of a slot `index` and a
//! `generation` counter. Allocation, removal, and slot reuse are handled by
//! [`entity_allocator::EntityAllocator`].
pub mod entity_allocator;

pub use entity_allocator::EntityAllocator;

/// A generational handle to an entity, made of a slot `index` and a
/// `generation` counter.
///
/// Two entities are equal only when both their `index` and `generation`
/// match. A handle captured before its slot is removed and reused compares
/// unequal to the handle later returned for that slot's new occupant, even
/// though both share the same `index`. This is what lets
/// [`entity_allocator::EntityAllocator::remove`] reject stale handles.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Entity {
    /// The slot this entity occupies in the allocator.
    index: u32,
    /// Distinguishes successive entities that have occupied the same slot.
    generation: u32,
}

impl Entity {
    /// Creates a handle for the given `index` and `generation`.
    ///
    /// This does not allocate a slot; entities intended for use with an
    /// [`entity_allocator::EntityAllocator`] should be obtained from its
    /// `create` method instead.
    pub fn new(index: u32, generation: u32) -> Self {
        Self { index, generation }
    }

    /// Returns the slot index this entity occupies.
    pub fn index(&self) -> u32 {
        self.index
    }

    /// Returns the generation of this entity's slot at the time the handle
    /// was created.
    pub fn generation(&self) -> u32 {
        self.generation
    }
}
