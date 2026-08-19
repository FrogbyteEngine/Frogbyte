//! Archetype storage for entities sharing the same component types.
//!
//! An archetype stores one type-erased column per component type and keeps
//! entity handles aligned with component rows.
//!
//! # Invariants
//!
//! - `key.components` is sorted and contains no duplicate component types.
//! - `columns` contains exactly one column for each component type in `key`.
//! - `columns` follows the same canonical type order as `key.components`.
//! - Every component column has the same logical length as `entities`.
//! - Row `i` of every column belongs to `entities[i]`.
//! - The empty archetype contains no component columns.

use std::any::TypeId;

use crate::{
    component::{Component, blobvec::BlobVec, component_set::ComponentSet},
    entity::Entity,
};

/// Stores entities that share one exact set of component types.
///
/// Components are stored in type-erased contiguous columns. A row is defined
/// by the same index across `entities` and every component column.
pub struct Archetype {
    key: ArchetypeKey,
    columns: Vec<BlobVec>,
    entities: Vec<Entity>,
}

/// Canonical identity of an archetype's component type set.
///
/// Component type identifiers are sorted at construction so equivalent sets
/// produce equal keys regardless of tuple ordering.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ArchetypeKey {
    components: Vec<TypeId>,
}

/// Ensures removed type-erased component values are destroyed during unwind.
///
/// Structural archetype changes are completed before component destructors are
/// invoked. If one destructor panics, the guard resumes destruction from the
/// next pending value while unwinding.
pub struct DropGuard {
    pending_drop: Vec<PendingDrop>,
    next: usize,
}

/// A type-erased component value awaiting destruction.
///
/// `ptr` identifies one initialized value that has already been removed from
/// its `BlobVec` logical range, and `drop_fn` is the destructor associated with
/// that value's concrete component type.
pub struct PendingDrop {
    ptr: *mut u8,
    drop_fn: unsafe fn(*mut u8),
}

impl DropGuard {
    /// Destroys the next pending component value.
    ///
    /// `next` is advanced before invoking the destructor so a panicking
    /// destructor is not attempted again during unwinding.
    fn drop_next(&mut self) {
        let index = self.next;

        self.next += 1;

        // SAFETY: [UNSAFE-023] Each pending entry pairs a pointer returned by
        // `BlobVec::raw_swap_remove` with the destructor from the same column.
        // The value is initialized, outside the BlobVec logical range, and has
        // not previously been destroyed. `next` is advanced first so this
        // destructor is never attempted twice if it panics.
        unsafe { (self.pending_drop[index].drop_fn)(self.pending_drop[index].ptr) };
    }
}

impl Drop for DropGuard {
    // Continue destroying pending values if normal processing was interrupted
    // by unwinding.
    fn drop(&mut self) {
        while self.next < self.pending_drop.len() {
            self.drop_next();
        }
    }
}

impl ArchetypeKey {
    /// Creates a canonical archetype key from component type identifiers.
    ///
    /// Identifier order does not affect the resulting key.
    ///
    /// # Panics
    ///
    /// Panics if the component set contains the same component type more than
    /// once.
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
    /// Creates an empty archetype for the component types in `T`.
    ///
    /// The key and component columns are created in canonical type order.
    pub fn new<T: ComponentSet>() -> Self {
        Self {
            key: ArchetypeKey::new(T::type_ids()),
            columns: T::create_column(),
            entities: Vec::new(),
        }
    }

    /// Appends an entity and its components as one aligned archetype row.
    ///
    /// Component tuple order does not need to match the archetype's canonical
    /// column order.
    ///
    /// # Panics
    ///
    /// Panics if the component set does not exactly match this archetype.
    pub fn insert<T: ComponentSet>(&mut self, entity: Entity, components: T) {
        let incoming_key = ArchetypeKey::new(T::type_ids());

        assert!(
            self.key == incoming_key,
            "ComponentSet does not match Archetype",
        );

        components.push_into(&mut self.columns);
        self.entities.push(entity);
    }

    /// Returns shared access to component `C` at `row_index`.
    ///
    /// The component column is located from the archetype's canonical key.
    ///
    /// # Panics
    ///
    /// Panics if `C` is not part of this archetype or `row_index` is out of
    /// bounds.
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

    /// Returns mutable access to component `C` at `row_index`.
    ///
    /// # Panics
    ///
    /// Panics if `C` is not part of this archetype or `row_index` is out of
    /// bounds.
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

    /// Removes an archetype row by replacing it with the final row.
    ///
    /// The same row is removed from every component column and from the entity
    /// column before any removed component destructor is invoked.
    ///
    /// Returns the entity moved into `row_index`, or `None` when the removed
    /// row was already the final row.
    ///
    /// # Panics
    ///
    /// Panics if `row_index` is out of bounds.
    ///
    /// Component destructors may also panic. Structural row alignment is
    /// restored before those destructors are executed.
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
