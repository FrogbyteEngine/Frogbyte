use std::{alloc::dealloc, any::TypeId, ops::Index};

use crate::{
    component::{Component, blobvec::BlobVec, component_set::ComponentSet},
    entity::Entity,
};

pub struct Archetype {
    key: ArchetypeKey,
    columns: Vec<BlobVec>,
    entities: Vec<Entity>,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ArchetypeKey {
    components: Vec<TypeId>,
}

pub struct DropGuard {
    pending_drop: Vec<PendingDrop>,
    next: usize,
}
pub struct PendingDrop {
    ptr: *mut u8,
    drop_fn: unsafe fn(*mut u8),
}

impl DropGuard {
    fn drop_next(&mut self) {
        let index = self.next;

        self.next += 1;

        unsafe { (self.pending_drop[index].drop_fn)(self.pending_drop[index].ptr) };
    }
}

impl Drop for DropGuard {
    fn drop(&mut self) {
        while self.next < self.pending_drop.len() {
            self.drop_next();
        }
    }
}

impl ArchetypeKey {
    pub fn new(mut component_ids: Vec<TypeId>) -> Self {
        component_ids.sort_unstable();

        assert!(
            component_ids.windows(2).all(|pair| pair[0] != pair[1]),
            "Archetype cannot contain duplicate component types",
        );

        Self {
            components: component_ids,
        }
    }
}

impl Archetype {
    pub fn new<T: ComponentSet>() -> Self {
        Self {
            key: ArchetypeKey::new(T::type_ids()),
            columns: T::create_column(),
            entities: Vec::new(),
        }
    }

    pub fn insert<T: ComponentSet>(&mut self, entity: Entity, components: T) {
        let incoming_key = ArchetypeKey::new(T::type_ids());

        assert!(
            self.key == incoming_key,
            "ComponentSet does not match Archetype",
        );

        components.push_into(&mut self.columns);
        self.entities.push(entity);
    }

    pub fn get<C: Component + 'static>(&self, row_index: usize) -> &C {
        let column_index = self
            .key
            .components
            .binary_search(&TypeId::of::<C>())
            .expect("Error: TypeId of this component does not exist in blobvec");

        &self.columns[column_index]
            .get(row_index)
            .expect("Error: Cannot get component at the corresponding index")
    }

    pub fn get_mut<C: Component + 'static>(&mut self, row_index: usize) -> &mut C {
        let column_index = self
            .key
            .components
            .binary_search(&TypeId::of::<C>())
            .expect("Error: TypeId of this component does not exist in blobvec");

        self.columns[column_index]
            .get_mut(row_index)
            .expect("Error: Cannot get component at the corresponding index")
    }

    pub fn swap_remove(&mut self, row_index: usize) -> Option<Entity> {
        assert!(row_index < self.entities.len());

        let mut drop_guard = DropGuard {
            pending_drop: Vec::with_capacity(self.columns.len()),
            next: 0,
        };

        for column in self.columns.iter_mut() {
            drop_guard.pending_drop.push(PendingDrop {
                ptr: column.raw_swap_remove(row_index),
                drop_fn: column.drop_fn(),
            });
        }

        self.entities.swap_remove(row_index);

        while drop_guard.next < drop_guard.pending_drop.len() {
            drop_guard.drop_next();
        }

        if row_index >= self.entities.len() {
            return None;
        }

        Some(self.entities[row_index])
    }
}
