use std::any::{TypeId};

use crate::component::{Component, blobvec::{BlobVec}};

pub trait ComponentSet {
    fn type_ids() -> Vec<TypeId>;
    fn push_into(self, columns: &mut [BlobVec]);
    fn create_column() -> Vec<BlobVec>;
}

macro_rules! ComponentSet_tuple_impl {
    ( $( $T:ident ),+ $(,)?) => {
        impl<$($T),+> ComponentSet for ($($T,)+)
        where
            $($T: Component + 'static),+
        {
            fn type_ids() -> Vec<TypeId> {
                let mut ids = vec![
                    $(TypeId::of::<$T>()),+
                ];

                ids.sort_unstable();
                ids
            }

            #[allow(non_snake_case)]
            fn push_into(self, columns: &mut [BlobVec]) {
                let ($($T,)+) = self;

                $( 
                    let column = columns
                        .iter_mut()
                        .find(|column| column.type_id() == TypeId::of::<$T>())
                        .expect("Error: Component column must exist in Archetype");
                
                    column.push($T);
                )+
            }

            fn create_column() -> Vec<BlobVec> {
                let mut columns = vec![
                    $(BlobVec::new::<$T>()),+
                ];

                columns.sort_unstable_by_key(|column| column.type_id());
                columns
            }
        }
    };  
}

ComponentSet_tuple_impl!(A,);
ComponentSet_tuple_impl!(A, B,);
ComponentSet_tuple_impl!(A, B, C,);
ComponentSet_tuple_impl!(A, B, C, D);
ComponentSet_tuple_impl!(A, B, C, D, E);
ComponentSet_tuple_impl!(A, B, C, D, E, F);