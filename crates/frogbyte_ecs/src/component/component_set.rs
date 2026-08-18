//! Heterogeneous component tuples used to create and populate archetypes.
//!
//! `ComponentSet` bridges statically typed Rust tuples with type-erased
//! archetype columns. Tuple ordering is independent from the canonical
//! component ordering used by an archetype.

use std::any::TypeId;

use crate::component::{Component, blobvec::BlobVec};

/// Describes a set of concrete component values used by an archetype.
///
/// Implementations provide the component type identities, create matching
/// type-erased columns, and route owned component values into those columns.
///
/// # Implementation contract
///
/// Implementations must:
///
/// - describe each component type exactly once;
/// - create exactly one `BlobVec` for each described component type;
/// - keep created columns in the same canonical type order used by
///   `ArchetypeKey`;
/// - insert exactly one value into its matching column for each component in
///   the set.
pub trait ComponentSet {
    /// Returns the concrete component types contained in this set.
    fn type_ids() -> Vec<TypeId>;

    /// Moves every component value into its matching archetype column.
    ///
    /// The provided columns must represent the same component type set.
    fn push_into(self, columns: &mut [BlobVec]);

    /// Creates one empty type-erased column for every component type in the
    /// set.
    ///
    /// Columns are returned in canonical component type order.
    fn create_column() -> Vec<BlobVec>;
}

// Generates ComponentSet implementations for heterogeneous component tuples.
//
// Type identifiers and created columns are canonicalized independently from
// tuple order, while `push_into` routes each concrete value by its TypeId.
macro_rules! ComponentSet_tuple_impl {
    () => {
        // The empty tuple represents the empty archetype and therefore has no
        // component types, columns, or values to insert.
        impl ComponentSet for () {
            fn type_ids() -> Vec<TypeId> {
                Vec::new()
            }

            fn push_into(self, _columns: &mut [BlobVec]) {
            }

            fn create_column() -> Vec<BlobVec> {
                Vec::new()
            }
        }
    };

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

ComponentSet_tuple_impl!();
ComponentSet_tuple_impl!(A,);
ComponentSet_tuple_impl!(A, B,);
ComponentSet_tuple_impl!(A, B, C,);
ComponentSet_tuple_impl!(A, B, C, D);
ComponentSet_tuple_impl!(A, B, C, D, E);
ComponentSet_tuple_impl!(A, B, C, D, E, F);
