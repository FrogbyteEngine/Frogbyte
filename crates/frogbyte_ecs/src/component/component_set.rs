use std::any::{TypeId};

use crate::component::{Component, blobvec::{BlobVec}};

pub trait ComponentSet {
    fn type_ids() -> Vec<TypeId>;
    fn push_into(self, columns: &mut [BlobVec]);
}

impl<A: Component + 'static> ComponentSet for (A,) {
    fn type_ids() -> Vec<TypeId> {
        let mut type_ids_storage = Vec::new();
        type_ids_storage.push(TypeId::of::<A>());
        type_ids_storage.sort();
        type_ids_storage
    }

    fn push_into(self, columns: &mut [BlobVec]) {
        let (a,) = self;

        let column = columns
            .iter_mut()
            .find(|column| column.type_id() == TypeId::of::<A>())
            .expect("Error: Component column must exist in Archetype");

        column.push(a);
    }
}

macro_rules! ComponentSet_tuple_impl {
    ( $( $T:ident )+ ) => {
        impl<$($T: Component + 'static),+> ComponentSet for ($($T,)+)
        {
            fn type_ids() -> Vec<TypeId> {
                let mut ids = vec![
                    $(TypeId::of::<$T>()),+
                ];

                ids.sort_unstable;
                ids
            }

            fn push_into(self, columns: &mut [BlobVec]) {
                let ($($name,)+) = self

                $( 
                    let column = columns
                        .iter_mut()
                        .find(|column| column.type_id() == TypeId::of::<$name>())
                        .expect("Error: Component column must exist in Archetype");
                
                    column.push($name);
                )+
            }
        }
    }
        
}