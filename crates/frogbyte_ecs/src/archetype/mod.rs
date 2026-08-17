use std::{any::TypeId, collections::HashMap};

use crate::{component::{Component, blobvec::BlobVec}, entity::Entity};

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
    pub fn new(mut components: Vec<TypeId>) -> Self {
        components.sort();

        Self { components }
    }
}

impl Archetype {
    pub fn new() -> Self {
        Self {
            column: Vec::new(),
            entities: Vec::new(),
        }
    }

    pub fn insert_or_create_row(&mut self, components: Vec<Component>) {
        // insert row if key exist
        // else create the new one and insert it
    } 
}

