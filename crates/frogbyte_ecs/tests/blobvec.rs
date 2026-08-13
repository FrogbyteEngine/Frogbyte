use std::{
    cell::RefCell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

use frogbyte_ecs::component::{Component, blobvec::BlobVec};

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq)]
struct Counter(i32);

impl Component for Counter {}

/// Deliberately has the same size and alignment as `Counter`.
///
/// Type-mismatch tests therefore exercise the `TypeId` boundary instead of
/// accidentally succeeding because the two component layouts differ.
#[repr(i32)]
#[derive(Clone, Copy)]
enum Alternate {
    Value = 1,
}

impl Component for Alternate {}

#[repr(align(64))]
struct AlignedTag;

impl Component for AlignedTag {}

struct DropRecorder {
    id: u32,
    drops: Rc<RefCell<Vec<u32>>>,
}

impl Component for DropRecorder {}

impl Drop for DropRecorder {
    fn drop(&mut self) {
        self.drops.borrow_mut().push(self.id);
    }
}

struct PanickingDrop {
    id: u32,
    panic_on_drop: bool,
    drops: Rc<RefCell<Vec<u32>>>,
}

impl Component for PanickingDrop {}

impl Drop for PanickingDrop {
    fn drop(&mut self) {
        self.drops.borrow_mut().push(self.id);

        if self.panic_on_drop {
            panic!("intentional component drop panic");
        }
    }
}

#[test]
fn new_blobvec_is_empty() {
    let mut blob = BlobVec::new::<Counter>();

    assert!(blob.get::<Counter>(0).is_none());
    assert!(blob.pop::<Counter>().is_none());
}

#[test]
fn push_then_get_returns_the_stored_value() {
    let mut blob = BlobVec::new::<Counter>();

    blob.push(Counter(42));

    assert_eq!(blob.get::<Counter>(0), Some(&Counter(42)));
}

#[test]
fn accessors_return_none_out_of_bounds() {
    let mut blob = BlobVec::new::<Counter>();
    blob.push(Counter(1));

    assert!(blob.get::<Counter>(1).is_none());
    assert!(blob.get_mut::<Counter>(1).is_none());
}

#[test]
fn get_mut_updates_the_stored_value() {
    let mut blob = BlobVec::new::<Counter>();
    blob.push(Counter(1));

    if let Some(value) = blob.get_mut::<Counter>(0) {
        value.0 = 42;
    } else {
        panic!("stored value should be accessible");
    }

    assert_eq!(blob.get::<Counter>(0), Some(&Counter(42)));
}

#[test]
fn growth_preserves_every_existing_value() {
    let mut blob = BlobVec::new::<Counter>();

    for value in 0..64 {
        blob.push(Counter(value));
    }

    for value in 0..64 {
        assert_eq!(blob.get::<Counter>(value as usize), Some(&Counter(value)));
    }
}

#[test]
fn pop_returns_values_in_reverse_insertion_order() {
    let mut blob = BlobVec::new::<Counter>();

    blob.push(Counter(1));
    blob.push(Counter(2));
    blob.push(Counter(3));

    assert_eq!(blob.pop::<Counter>(), Some(Counter(3)));
    assert_eq!(blob.pop::<Counter>(), Some(Counter(2)));
    assert_eq!(blob.pop::<Counter>(), Some(Counter(1)));
    assert_eq!(blob.pop::<Counter>(), None);
}

#[test]
fn swap_remove_moves_the_last_element_into_the_removed_slot() {
    let mut blob = BlobVec::new::<Counter>();

    blob.push(Counter(10));
    blob.push(Counter(20));
    blob.push(Counter(30));
    blob.push(Counter(40));

    let removed = blob.swap_remove::<Counter>(1);

    assert_eq!(removed, Counter(20));
    assert_eq!(blob.get::<Counter>(0), Some(&Counter(10)));
    assert_eq!(blob.get::<Counter>(1), Some(&Counter(40)));
    assert_eq!(blob.get::<Counter>(2), Some(&Counter(30)));
    assert!(blob.get::<Counter>(3).is_none());
}

#[test]
fn swap_remove_last_element_only_shrinks_the_storage() {
    let mut blob = BlobVec::new::<Counter>();

    blob.push(Counter(1));
    blob.push(Counter(2));
    blob.push(Counter(3));

    let removed = blob.swap_remove::<Counter>(2);

    assert_eq!(removed, Counter(3));
    assert_eq!(blob.get::<Counter>(0), Some(&Counter(1)));
    assert_eq!(blob.get::<Counter>(1), Some(&Counter(2)));
    assert!(blob.get::<Counter>(2).is_none());
}

#[test]
fn interleaved_operations_preserve_storage_state() {
    let mut blob = BlobVec::new::<Counter>();

    blob.push(Counter(1));
    blob.push(Counter(2));
    blob.push(Counter(3));
    blob.push(Counter(4));

    assert_eq!(blob.swap_remove::<Counter>(0), Counter(1));
    assert_eq!(blob.pop::<Counter>(), Some(Counter(3)));

    blob.push(Counter(5));

    assert_eq!(blob.get::<Counter>(0), Some(&Counter(4)));
    assert_eq!(blob.get::<Counter>(1), Some(&Counter(2)));
    assert_eq!(blob.get::<Counter>(2), Some(&Counter(5)));
    assert!(blob.get::<Counter>(3).is_none());
}

#[test]
#[should_panic]
fn swap_remove_panics_out_of_bounds() {
    let mut blob = BlobVec::new::<Counter>();
    blob.push(Counter(1));

    blob.swap_remove::<Counter>(1);
}

#[test]
#[should_panic]
fn push_rejects_a_different_component_type_with_the_same_layout() {
    let mut blob = BlobVec::new::<Counter>();

    blob.push(Alternate::Value);
}

#[test]
#[should_panic]
fn get_rejects_a_different_component_type_with_the_same_layout() {
    let mut blob = BlobVec::new::<Counter>();
    blob.push(Counter(1));

    let _ = blob.get::<Alternate>(0);
}

#[test]
#[should_panic]
fn get_mut_rejects_a_different_component_type_with_the_same_layout() {
    let mut blob = BlobVec::new::<Counter>();
    blob.push(Counter(1));

    let _ = blob.get_mut::<Alternate>(0);
}

#[test]
#[should_panic]
fn pop_rejects_a_different_component_type_with_the_same_layout() {
    let mut blob = BlobVec::new::<Counter>();
    blob.push(Counter(1));

    let _ = blob.pop::<Alternate>();
}

#[test]
#[should_panic]
fn swap_remove_rejects_a_different_component_type_with_the_same_layout() {
    let mut blob = BlobVec::new::<Counter>();
    blob.push(Counter(1));

    let _ = blob.swap_remove::<Alternate>(0);
}

#[test]
fn dropping_blobvec_drops_every_remaining_value_exactly_once() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let mut blob = BlobVec::new::<DropRecorder>();

    for id in 0..5 {
        blob.push(DropRecorder {
            id,
            drops: Rc::clone(&drops),
        });
    }

    drop(blob);

    let mut recorded = drops.borrow().clone();
    recorded.sort_unstable();

    assert_eq!(recorded, vec![0, 1, 2, 3, 4]);
}

#[test]
fn pop_transfers_ownership_without_double_drop() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let mut blob = BlobVec::new::<DropRecorder>();

    blob.push(DropRecorder {
        id: 7,
        drops: Rc::clone(&drops),
    });

    let popped = blob.pop::<DropRecorder>();

    assert!(
        drops.borrow().is_empty(),
        "pop must not drop the returned component"
    );

    drop(popped);
    assert_eq!(*drops.borrow(), vec![7]);

    drop(blob);

    assert_eq!(
        *drops.borrow(),
        vec![7],
        "BlobVec must not drop a component already returned by pop"
    );
}

#[test]
fn swap_remove_transfers_ownership_without_double_drop() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let mut blob = BlobVec::new::<DropRecorder>();

    for id in 1..=3 {
        blob.push(DropRecorder {
            id,
            drops: Rc::clone(&drops),
        });
    }

    let removed = blob.swap_remove::<DropRecorder>(0);

    assert!(
        drops.borrow().is_empty(),
        "swap_remove must not drop the returned component"
    );

    drop(removed);
    assert_eq!(*drops.borrow(), vec![1]);

    drop(blob);

    let mut recorded = drops.borrow().clone();
    recorded.sort_unstable();

    assert_eq!(recorded, vec![1, 2, 3]);
}

#[test]
fn drop_cleans_remaining_components_when_a_destructor_panics() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let mut blob = BlobVec::new::<PanickingDrop>();

    blob.push(PanickingDrop {
        id: 1,
        panic_on_drop: true,
        drops: Rc::clone(&drops),
    });
    blob.push(PanickingDrop {
        id: 2,
        panic_on_drop: false,
        drops: Rc::clone(&drops),
    });
    blob.push(PanickingDrop {
        id: 3,
        panic_on_drop: false,
        drops: Rc::clone(&drops),
    });

    let result = catch_unwind(AssertUnwindSafe(|| {
        drop(blob);
    }));

    assert!(result.is_err());

    let mut recorded = drops.borrow().clone();
    recorded.sort_unstable();

    assert_eq!(
        recorded,
        vec![1, 2, 3],
        "remaining initialized components must be cleaned during unwinding"
    );
}

#[test]
fn aligned_zero_sized_components_are_supported() {
    let mut blob = BlobVec::new::<AlignedTag>();

    blob.push(AlignedTag);
    blob.push(AlignedTag);
    blob.push(AlignedTag);

    assert!(blob.get::<AlignedTag>(0).is_some());
    assert!(blob.get::<AlignedTag>(2).is_some());
    assert!(blob.get::<AlignedTag>(3).is_none());

    let _ = blob.swap_remove::<AlignedTag>(1);

    assert!(blob.get::<AlignedTag>(0).is_some());
    assert!(blob.get::<AlignedTag>(1).is_some());
    assert!(blob.get::<AlignedTag>(2).is_none());

    assert!(blob.pop::<AlignedTag>().is_some());
    assert!(blob.pop::<AlignedTag>().is_some());
    assert!(blob.pop::<AlignedTag>().is_none());
}
