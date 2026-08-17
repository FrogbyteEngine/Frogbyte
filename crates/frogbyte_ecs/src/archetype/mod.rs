use std::{any::TypeId, ops::Index};

use crate::{component::{Component, blobvec::BlobVec, component_set::ComponentSet}, entity::Entity};

pub struct Archetype {
    key: ArchetypeKey,
    column: Vec<BlobVec>,
    entities: Vec<Entity>,
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ArchetypeKey {
    components: Vec<TypeId>,
}

impl ArchetypeKey {
    pub fn new(mut component_ids: Vec<TypeId>) -> Self {
        component_ids.sort_unstable();

        assert!(
            component_ids.windows(2).all(|pair| pair[0] != pair[1]),
            "Archetype cannot contain duplicate component types",
        );

        Self { components: component_ids }
    }
}

impl Archetype {
    pub fn new<T:ComponentSet>() -> Self {
        Self {
            key: ArchetypeKey::new(T::type_ids()),
            column: T::create_column(),
            entities: Vec::new(),
        }
    }

    pub fn insert<T: ComponentSet>(&mut self, entity: Entity, components: T) {
        let incoming_key = ArchetypeKey::new(T::type_ids());

        assert!(
            self.key == incoming_key,
            "ComponentSet does not match Archetype",
        );

        components.push_into(&mut self.column);
        self.entities.push(entity);
    }

    pub fn get<C: Component + 'static>(&self, row_index: usize) -> &C {
        let column_index = self
            .key
            .components
            .binary_search(&TypeId::of::<C>())
            .expect("Error: TypeId of this component does not exist in blobvec");

        &self.column[column_index].get(row_index).expect("Error: Cannot get component at the corresponding index")
    }

    pub fn get_mut<C: Component + 'static>(&mut self, row_index: usize) -> &mut C {
         let column_index = self
            .key
            .components
            .binary_search(&TypeId::of::<C>())
            .expect("Error: TypeId of this component does not exist in blobvec");

        self.column[column_index].get_mut(row_index).expect("Error: Cannot get component at the corresponding index")
    }
}